use anyhow::Context as _;
use core::fmt;
use indexmap::IndexSet;
use itertools::Itertools as _;
use mail_avatar::{AvatarInformation, proton_color};
use mail_contacts_api::ContactGroupId;
use mail_core_api::services::proton::{ContactId, PrivateEmail};
use mail_core_api::session::Session;
use mail_shared_types::{MapVec as _, ModelExtension};
use mail_stash::orm::Model as _;
use mail_stash::stash::Tether;
use mail_vcard::address::Address as VcardAddress;
use mail_vcard::categories::Category;
use mail_vcard::email::Email;
use mail_vcard::gender::GenderValue;
use mail_vcard::parameters::type_generic::GenericType;
use mail_vcard::parameters::type_tel::TelType;
use mail_vcard::values::date_and_or_time::MaybeDateAndOrTime;
use mail_vcard::values::uri::MaybeUri;
use mail_vcard::vcard::{PropertyUid, ToSorted, VCard};
use proton_crypto::crypto::PGPProviderSync;
use proton_crypto_account::keys::UnlockedUserKeys;
use std::collections::HashMap;
use std::fmt::Display;
use tracing::warn;
use url::Url;

use crate::contact::Contact;
use crate::contact_email::ContactEmail;
use crate::contact_group::{ContactGroup as ContactGroupModel, ContactGroupColor};
use crate::local_ids::LocalContactId;

/// Represents some data known from the vCard in a form more suitable for human consumption than a
/// raw vcard.
/// These are meant to be used directly by the clients and it sort of represents data in a view.
#[derive(Clone, Debug)]
pub struct InspectableContactDetails {
    /// Clients want this for consistency
    pub id: LocalContactId,
    pub remote_id: Option<ContactId>,
    pub avatar_information: AvatarInformation,
    pub extended_name: ExtendedName,
    /// These are sorted per display order
    pub fields: Vec<ContactField>,
}

// These are ordered by display order! Please be careful before moving them around.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ContactField {
    Emails(Vec<ContactDetailsEmail>),
    Phones(Vec<Telephone>),
    Address(Vec<ContactDetailAddress>),
    Birthday(MaybeDateAndOrTime),
    Notes(Vec<String>),
    Anniversary(MaybeDateAndOrTime),
    Gender(Gender),
    Languages(Vec<String>),
    TimeZones(Vec<String>),
    Titles(Vec<String>),
    Roles(Vec<String>),
    Logos(Vec<Url>),
    Photos(Vec<Url>),
    Organizations(Vec<String>),
    Members(Vec<String>),
    Urls(Vec<VCardUrl>),
}

#[cfg(feature = "test-utils")]
impl Display for InspectableContactDetails {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:#?}")
    }
}

impl InspectableContactDetails {
    pub async fn get_from_contact<T: PGPProviderSync>(
        session: &Session,
        provider: &T,
        keys: &UnlockedUserKeys<T>,
        contact_id: LocalContactId,
        tether: &mut Tether,
    ) -> anyhow::Result<Self> {
        match Self::get_from_contact_full(session, provider, keys, contact_id, tether).await {
            Ok(details) => Ok(details),
            Err(e) => {
                warn!(
                    "Failed to get contact details from contact: {e:?}. Falling back to basic contact data"
                );

                let contact = Contact::load(contact_id, tether)
                    .await?
                    .context("Contact does not exist")?;
                let contact_groups = ContactGroupModel::all(tether).await?;

                Ok(Self::get_from_contact_basic(contact, &contact_groups))
            }
        }
    }

    #[must_use]
    pub fn get_from_contact_basic(contact: Contact, contact_groups: &[ContactGroupModel]) -> Self {
        let id = contact.id();
        let remote_id = contact.remote_id;

        let emails = contact
            .contact_emails
            .into_iter()
            .map(|contact_email| ContactDetailsEmail {
                email_type: vec![],
                email: contact_email.email.clone(),
                groups: Self::matching_contact_groups(contact_groups, &contact_email),
            })
            .collect();

        Self {
            id,
            remote_id,
            avatar_information: AvatarInformation::from(&contact.name),
            extended_name: ExtendedName {
                first: Some(contact.name),
                ..Default::default()
            },
            fields: vec![ContactField::Emails(emails)],
        }
    }

    fn matching_contact_groups(
        contact_groups: &[ContactGroupModel],
        contact_email: &ContactEmail,
    ) -> Vec<ContactGroup> {
        let groups_map: HashMap<&str, &ContactGroupModel> = contact_groups
            .iter()
            .filter_map(|group| group.remote_id.as_deref().map(|id| (id, group)))
            .collect();
        let unique_contact_group_ids: IndexSet<&ContactGroupId> =
            contact_email.label_ids.iter().collect();

        unique_contact_group_ids
            .iter()
            .filter_map(|contact_group_id| groups_map.get(contact_group_id.as_str()))
            .map(|group| ContactGroup {
                name: group.name.clone(),
                color: group.color.clone(),
            })
            .collect()
    }

    async fn get_from_contact_full<T: PGPProviderSync>(
        session: &Session,
        provider: &T,
        keys: &UnlockedUserKeys<T>,
        contact_id: LocalContactId,
        tether: &mut Tether,
    ) -> anyhow::Result<Self> {
        Contact::sync_with_card(contact_id, session, tether).await?;

        let contact = Contact::load(contact_id, tether)
            .await?
            .context("Contact does not exist")?;

        let vcard = contact.vcard_details(tether, provider, keys).await?;

        Ok(Self::from_vcard(contact, vcard))
    }

    /// Transforms the data in the vCard struct to something suitable for human consumption
    pub fn from_vcard(contact: Contact, vcard: VCard) -> Self {
        let mut result = Self::get_from_contact_basic(contact, &[]);
        let fields = &mut result.fields;

        match &mut fields[0] {
            ContactField::Emails(emails) => {
                *emails = Self::emails(vcard.emails, &vcard.categories);
            }
            _ => unreachable!("The first and only field should always be the emails field"),
        }

        vcard
            .telephones
            .sorted_extend(fields, ContactField::Phones, |tel| Telephone {
                number: tel.value.to_string(),
                tel_types: tel.tel_type.iter().cloned().map_vec(),
            });

        vcard
            .addresses
            .sorted_extend(fields, ContactField::Address, ContactDetailAddress::from);

        if let Some(name) = vcard.name {
            result.extended_name = ExtendedName {
                last: name.last.concat_to_string(" "),
                first: name.first.concat_to_string(" "),
                additional: name.additional.concat_to_string(" "),
                prefix: name.prefix.concat_to_string(" "),
                suffix: name.suffix.concat_to_string(" "),
            };
        }

        if let Some(g) = vcard.gender {
            fields.push(ContactField::Gender(g.value.into()));
        }
        if let Some(g) = vcard.anniversary {
            fields.push(ContactField::Anniversary(g.value));
        }
        if let Some(g) = vcard.birthday {
            fields.push(ContactField::Birthday(g.value));
        }

        vcard
            .urls
            .sorted_extend(fields, ContactField::Urls, |u| VCardUrl {
                url_type: u.r#type.into_iter().map_vec(),
                url: u.value.into(),
            });
        vcard
            .organizations
            .sorted_extend(fields, ContactField::Organizations, |x| {
                x.values.into_iter().join(", ")
            });

        let logos = vcard
            .logos
            .to_sorted_iter(|v| v.value.clone())
            .filter_map(|v| {
                if is_safe_image_uri(&v.0) {
                    Some(v.0)
                } else {
                    warn!("{} is not a safe logo url, removing from list", v.0);
                    None
                }
            })
            .collect::<Vec<_>>();
        if !logos.is_empty() {
            fields.push(ContactField::Logos(logos));
        }

        let photos = vcard
            .photos
            .to_sorted_iter(|v| v.value.clone())
            .filter_map(|v| {
                if is_safe_image_uri(&v.0) {
                    Some(v.0)
                } else {
                    warn!("{} is not a safe photo url, removing from list", v.0);
                    None
                }
            })
            .collect::<Vec<_>>();
        if !photos.is_empty() {
            fields.push(ContactField::Photos(photos));
        }

        vcard
            .time_zones
            .sorted_extend(fields, ContactField::TimeZones, |x| x.value.to_string());
        vcard
            .notes
            .sorted_extend(fields, ContactField::Notes, |x| x.value.value);
        vcard
            .titles
            .sorted_extend(fields, ContactField::Titles, |x| x.value.value);
        vcard
            .roles
            .sorted_extend(fields, ContactField::Roles, |x| x.value.value);
        vcard
            .languages
            .sorted_extend(fields, ContactField::Languages, |x| x.value);
        vcard
            .members
            .sorted_extend(fields, ContactField::Members, |x| x.value.to_string());

        // Very important that this is a stable sort!
        fields.sort();

        result
    }

    fn emails(
        vcard_emails: HashMap<PropertyUid, Email>,
        vcard_categories: &HashMap<PropertyUid, Category>,
    ) -> Vec<ContactDetailsEmail> {
        vcard_emails
            .to_sorted_iter(|email| ContactDetailsEmail {
                email_type: email.r#type.iter().cloned().map_vec(),
                email: email.value.value.clone().into(),
                groups: Self::groups(vcard_categories, &email),
            })
            .collect()
    }

    fn groups(
        vcard_categories: &HashMap<PropertyUid, Category>,
        email: &Email,
    ) -> Vec<ContactGroup> {
        let matching_categories: Vec<&Category> = vcard_categories
            .values()
            .filter(|category| category.group == email.group)
            .collect();
        matching_categories
            .iter()
            .flat_map(|category| {
                category.value.0.iter().map(|category_name| ContactGroup {
                    name: category_name.value.clone(),
                    color: proton_color(&category_name.value).into(),
                })
            })
            .collect()
    }
}

#[derive(Clone, Default, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ExtendedName {
    pub last: Option<String>,
    pub first: Option<String>,
    pub additional: Option<String>,
    pub prefix: Option<String>,
    pub suffix: Option<String>,
}

#[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord)]
pub struct ContactDetailAddress {
    pub street: Option<String>,
    pub city: Option<String>,
    pub region: Option<String>,
    pub postal_code: Option<String>,
    pub country: Option<String>,
    pub addr_type: Vec<VcardPropType>,
}

impl From<VcardAddress> for ContactDetailAddress {
    fn from(value: VcardAddress) -> Self {
        Self {
            street: value.street.concat_to_string(", "),
            city: value.locality.concat_to_string(", "),
            region: value.region.concat_to_string(", "),
            postal_code: value.postal_code.concat_to_string(", "),
            country: value.country.concat_to_string(", "),
            addr_type: value.r#type.map_vec(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord)]
pub struct Telephone {
    pub number: String,
    pub tel_types: Vec<VcardPropType>,
}

#[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord)]
pub struct VCardUrl {
    pub url: VCardUrlValue,
    pub url_type: Vec<VcardPropType>,
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Eq, Ord)]
pub enum VCardUrlValue {
    Http(url::Url),
    NotHttp(url::Url),
    Text(String),
}

impl Display for VCardUrlValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VCardUrlValue::Http(v) | VCardUrlValue::NotHttp(v) => fmt::Display::fmt(v, f),
            VCardUrlValue::Text(v) => fmt::Display::fmt(v, f),
        }
    }
}

impl From<MaybeUri> for VCardUrlValue {
    fn from(value: MaybeUri) -> Self {
        match value {
            MaybeUri::Uri(uri) => {
                let scheme = uri.scheme();
                if scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https") {
                    Self::Http(uri)
                } else {
                    Self::NotHttp(uri)
                }
            }
            MaybeUri::Text(v) => Self::Text(v.clone()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ContactDetailsEmail {
    pub email_type: Vec<VcardPropType>,
    pub email: PrivateEmail,
    pub groups: Vec<ContactGroup>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ContactGroup {
    pub name: String,
    pub color: ContactGroupColor,
}

#[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord)]
pub enum VcardPropType {
    Home,
    Work,
    Text,
    Voice,
    Fax,
    Cell,
    Video,
    Pager,
    TextPhone,
    String(String),
}

impl Display for VcardPropType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VcardPropType::Home => write!(f, "home"),
            VcardPropType::Work => write!(f, "work"),
            VcardPropType::Text => write!(f, "text"),
            VcardPropType::Voice => write!(f, "voice"),
            VcardPropType::Fax => write!(f, "fax"),
            VcardPropType::Cell => write!(f, "cell"),
            VcardPropType::Video => write!(f, "video"),
            VcardPropType::Pager => write!(f, "pager"),
            VcardPropType::TextPhone => write!(f, "textphone"),
            VcardPropType::String(s) => write!(f, "{s}"),
        }
    }
}

impl From<GenericType> for VcardPropType {
    fn from(value: GenericType) -> Self {
        match value {
            GenericType::Home => VcardPropType::Home,
            GenericType::Work => VcardPropType::Work,
            GenericType::IanaToken(tok) => VcardPropType::String(tok.0),
            GenericType::XName(xname) => VcardPropType::String(xname.0),
        }
    }
}

impl From<TelType> for VcardPropType {
    fn from(value: TelType) -> Self {
        match value {
            TelType::Home => VcardPropType::Home,
            TelType::Work => VcardPropType::Work,
            TelType::Text => VcardPropType::Text,
            TelType::Voice => VcardPropType::Voice,
            TelType::Fax => VcardPropType::Fax,
            TelType::Cell => VcardPropType::Cell,
            TelType::Video => VcardPropType::Video,
            TelType::Pager => VcardPropType::Pager,
            TelType::TextPhone => VcardPropType::TextPhone,
            TelType::IanaToken(tok) => VcardPropType::String(tok.0),
            TelType::XName(xname) => VcardPropType::String(xname.0),
        }
    }
}

fn is_safe_image_uri(url: &Url) -> bool {
    let scheme = url.scheme();
    scheme.eq_ignore_ascii_case("http")
        || scheme.eq_ignore_ascii_case("https")
        || scheme.eq_ignore_ascii_case("data")
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Gender {
    Male,
    Female,
    Other,
    NotApplicable,
    Unknown,
    None,
    String(String),
}

impl Display for Gender {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Gender::Male => write!(f, "male"),
            Gender::Female => write!(f, "female"),
            Gender::Other => write!(f, "other"),
            Gender::NotApplicable => write!(f, "N/A"),
            Gender::Unknown => write!(f, "unknown"),
            Gender::None => write!(f, "none"),
            Gender::String(value) => write!(f, "{value}"),
        }
    }
}

impl From<GenderValue> for Gender {
    fn from(value: GenderValue) -> Self {
        match value {
            GenderValue::Male(_) => Gender::Male,
            GenderValue::Female(_) => Gender::Female,
            GenderValue::Other(_) => Gender::Other,
            GenderValue::NotApplicable(_) => Gender::NotApplicable,
            GenderValue::Unknown(_) => Gender::Unknown,
            GenderValue::None(_) => Gender::None,
            GenderValue::Custom(value) => Gender::String(value),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contact::Contact;
    use crate::contact_email::ContactEmail;
    use crate::local_ids::LocalContactId;
    use bytes::Buf as _;
    use ical::VcardParser;
    use insta::assert_snapshot;
    use mail_core_api::services::proton::{ContactId, PrivateEmail};
    use mail_vcard::vcard::VCard;
    use std::fmt::Display;

    #[allow(unused, reason = "The fields are only used for their debug impl")]
    #[derive(Debug)]
    struct Snapshot {
        vcard: &'static str,
        fields: Vec<ContactField>,
    }
    impl Display for Snapshot {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            writeln!(f, "VCARD:")?;
            writeln!(f, "{}", self.vcard)?;
            writeln!(f, "---------------------------\n")?;
            writeln!(f, "Sorted fields:")?;
            for field in &self.fields {
                writeln!(f, "{field:?}")?;
            }
            Ok(())
        }
    }

    fn get_vcard(raw_vcard: &'static str) -> Snapshot {
        let mut r = VcardParser::new(raw_vcard.as_bytes().reader());
        let c = r.next().expect("Expected 1 card").unwrap();
        assert!(r.next().is_none(), "Expected exactly 1 card");
        let vcard = VCard::from_ical_contact(c).unwrap();
        let contact = Contact {
            local_id: Some(LocalContactId::from(42_u64)),
            remote_id: Some(ContactId::new("remote_id_42".to_string())),
            ..Contact::test_default()
        };
        Snapshot {
            vcard: raw_vcard,
            fields: InspectableContactDetails::from_vcard(contact, vcard).fields,
        }
    }

    #[allow(clippy::similar_names)]
    #[test]
    fn get_from_contact_basic() {
        let group_a_id = ContactGroupId::from("<group_a_id>");
        let group_b_id = ContactGroupId::from("<group_b_id>");
        let group_c_id = ContactGroupId::from("<group_c_id>");
        let contact = Contact {
            local_id: Some(LocalContactId::from(1_u64)),
            remote_id: Some(ContactId::from("42")),
            name: "Peter Parker".to_string(),
            contact_emails: vec![
                test_contact_email("peter@pm.me", &[group_a_id.as_str(), group_b_id.as_str()]),
                test_contact_email("peter@gmail.com", &[group_c_id.as_str()]),
                test_contact_email("peter.parker@proton.me", &[]),
            ],
            ..Contact::test_default()
        };
        let contact_groups: Vec<ContactGroupModel> = vec![
            test_contact_group(&group_a_id, "A"),
            test_contact_group(&group_b_id, "B"),
            test_contact_group(&group_c_id, "C"),
        ];

        let contact_details =
            InspectableContactDetails::get_from_contact_basic(contact, &contact_groups);

        assert_snapshot!(&contact_details);
    }

    fn test_contact_email(email: &str, label_ids: &[&str]) -> ContactEmail {
        ContactEmail {
            email: PrivateEmail::from(email),
            label_ids: label_ids
                .iter()
                .map(|label_id| ContactGroupId::from(*label_id))
                .collect(),
            ..ContactEmail::test_default()
        }
    }

    fn test_contact_group(remote_id: &ContactGroupId, name: &str) -> ContactGroupModel {
        ContactGroupModel {
            remote_id: Some(remote_id.clone()),
            name: name.to_owned(),
            ..ContactGroupModel::test_default()
        }
    }

    #[test]
    fn real_contact() {
        let real = include_str!("../tests/vcards/real.vcf");
        assert_snapshot!(get_vcard(real));
    }
    #[test]
    fn real_autosave() {
        let real_autosave = include_str!("../tests/vcards/real-autosave.vcf");
        assert_snapshot!(get_vcard(real_autosave));
    }

    #[test]
    fn full() {
        let full = include_str!("../tests/vcards/full.vcf");
        assert_snapshot!(get_vcard(full));
    }

    #[test]
    fn small() {
        let small = include_str!("../tests/vcards/small.vcf");
        assert_snapshot!(get_vcard(small));
    }

    #[test]
    fn vcard_v3() {
        let v3 = include_str!("../tests/vcards/v3.vcf");
        assert_snapshot!(get_vcard(v3));
    }

    #[test]
    fn frodo() {
        let frodo = include_str!("../tests/vcards/frodo.vcf");
        assert_snapshot!(get_vcard(frodo));
    }

    #[test]
    fn mateusz() {
        let mateusz = include_str!("../tests/vcards/mateusz.vcf");
        assert_snapshot!(get_vcard(mateusz));
    }
}
