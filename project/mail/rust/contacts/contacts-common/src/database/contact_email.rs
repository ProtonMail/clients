use contact_database::{RoContactEmailTable, RwContactEmailTable};
use mail_contacts_api::{ContactEmailId, ContactGroupId};
use mail_shared_types::ModelExtension;
use mail_stash::exports::Connection;
use mail_stash::orm::{Model, ModelRaw};
use mail_stash::stash::StashError;

use crate::contact_email::ContactEmail;
use crate::contact_group::ContactGroup;
use crate::database::{ContactReadTx, ContactWriteTx};
use crate::local_ids::{LocalContactEmailId, LocalContactGroupId};
use crate::types::ContactTypes;

impl RoContactEmailTable for ContactReadTx<'_> {
    type Error = StashError;

    async fn find_contact_email_by_id(
        &self,
        id: contact_database::LocalContactEmailId,
    ) -> Result<Option<contact_database::ContactEmail>, Self::Error> {
        self.0
            .sync_query(move |conn| find_contact_email_by_id(conn, id.into()))
            .await
    }

    async fn find_contact_email_by_remote_id(
        &self,
        id: &mail_contacts_api::ContactEmailId,
    ) -> Result<Option<contact_database::ContactEmail>, Self::Error> {
        let id = id.clone();
        self.0
            .sync_query(move |conn| find_contact_email_by_remote_id(conn, &id))
            .await
    }

    async fn count_contact_emails_in_group_by_name(
        &self,
        name: &str,
    ) -> Result<usize, Self::Error> {
        let name = name.to_owned();
        self.0
            .sync_query(move |conn| count_contact_emails_in_group_with_name(conn, &name))
            .await
    }

    async fn count_contact_emails_in_group(
        &self,
        contact_group_id: contact_database::LocalContactGroupId,
    ) -> Result<usize, Self::Error> {
        self.0
            .sync_query(move |conn| {
                count_contact_emails_in_group_with_local_id(conn, contact_group_id.into())
            })
            .await
    }

    async fn count_contact_emails_in_group_with_remote_id(
        &self,
        contact_group_id: &mail_contacts_api::ContactGroupId,
    ) -> Result<usize, Self::Error> {
        let contact_group_id = contact_group_id.clone();
        self.0
            .sync_query(move |conn| {
                count_contact_emails_in_group_with_remote_id(conn, &contact_group_id)
            })
            .await
    }
}

impl RoContactEmailTable for ContactWriteTx<'_> {
    type Error = StashError;

    async fn find_contact_email_by_id(
        &self,
        id: contact_database::LocalContactEmailId,
    ) -> Result<Option<contact_database::ContactEmail>, Self::Error> {
        self.0
            .sync_query(move |conn| find_contact_email_by_id(conn, id.into()))
            .await
    }

    async fn find_contact_email_by_remote_id(
        &self,
        id: &mail_contacts_api::ContactEmailId,
    ) -> Result<Option<contact_database::ContactEmail>, Self::Error> {
        let id = id.clone();
        self.0
            .sync_query(move |conn| find_contact_email_by_remote_id(conn, &id))
            .await
    }

    async fn count_contact_emails_in_group_by_name(
        &self,
        name: &str,
    ) -> Result<usize, Self::Error> {
        let name = name.to_owned();
        self.0
            .sync_query(move |conn| count_contact_emails_in_group_with_name(conn, &name))
            .await
    }

    async fn count_contact_emails_in_group(
        &self,
        contact_group_id: contact_database::LocalContactGroupId,
    ) -> Result<usize, Self::Error> {
        self.0
            .sync_query(move |conn| {
                count_contact_emails_in_group_with_local_id(conn, contact_group_id.into())
            })
            .await
    }

    async fn count_contact_emails_in_group_with_remote_id(
        &self,
        contact_group_id: &mail_contacts_api::ContactGroupId,
    ) -> Result<usize, Self::Error> {
        let contact_group_id = contact_group_id.clone();
        self.0
            .sync_query(move |conn| {
                count_contact_emails_in_group_with_remote_id(conn, &contact_group_id)
            })
            .await
    }
}

impl RwContactEmailTable for ContactWriteTx<'_> {
    async fn crate_contact_email(
        &self,
        mut contact_email: contact_database::NewContactEmail,
    ) -> Result<contact_database::ContactEmail, Self::Error> {
        self.0
            .sync_bridge(move |tx| {
                let contact_group_ids = std::mem::take(&mut contact_email.label_ids)
                    .into_iter()
                    .map(Into::into)
                    .collect::<Vec<_>>();
                let mut contact_email: ContactEmail = contact_email.into();
                contact_email.save_raw_sync(tx)?;
                ContactGroup::relink_contact_groups_for_contact_email_local_id(
                    tx,
                    contact_email.id(),
                    &contact_group_ids,
                )?;

                let mut contact_email: contact_database::ContactEmail = contact_email.into();
                contact_email.label_ids = contact_group_ids.into_iter().map(Into::into).collect();

                Ok(contact_email)
            })
            .await
    }

    async fn upsert_contact_email(
        &self,
        contact_email: contact_database::UpseratableContactEmail,
    ) -> Result<contact_database::ContactEmail, Self::Error> {
        self.0
            .sync_bridge(move |tx| {
                // run full model with hooks for this request.
                let mut contact_email: ContactEmail = contact_email.into();
                contact_email.save_sync(tx)?;

                let mut contact_email: contact_database::ContactEmail = contact_email.into();
                load_contact_email_groups(tx, std::slice::from_mut(&mut contact_email))?;

                Ok(contact_email)
            })
            .await
    }

    async fn upsert_contact_emails(
        &self,
        contact_emails: impl IntoIterator<Item = contact_database::UpseratableContactEmail>,
    ) -> Result<Vec<contact_database::ContactEmail>, Self::Error> {
        let mut contact_emails: Vec<ContactEmail> =
            contact_emails.into_iter().map(Into::into).collect();
        self.0
            .sync_bridge(move |tx| {
                // run full model with hooks for this request.
                for contact_email in &mut contact_emails {
                    contact_email.save_sync(tx)?;
                }

                let mut contact_emails = contact_emails
                    .into_iter()
                    .map(Into::into)
                    .collect::<Vec<_>>();
                load_contact_email_groups(tx, &mut contact_emails)?;

                Ok(contact_emails)
            })
            .await
    }

    async fn update_contact_email(
        &self,
        contact_email: &contact_database::ContactEmail,
    ) -> Result<(), Self::Error> {
        let contact_group_ids = contact_email
            .label_ids
            .clone()
            .into_iter()
            .map(Into::into)
            .collect::<Vec<_>>();
        let mut contact_email: ContactEmail = contact_email.into();
        self.0
            .sync_bridge(move |tx| {
                contact_email.save_raw_sync(tx)?;
                ContactGroup::link_contact_groups_for_contact_email_with_local_id(
                    tx,
                    contact_email.id(),
                    &contact_group_ids,
                )?;
                Ok(())
            })
            .await
    }

    async fn delete_contact_emails(
        &self,
        ids: impl IntoIterator<Item = contact_database::LocalContactEmailId>,
    ) -> Result<(), Self::Error> {
        let ids = ids.into_iter().map(Into::into).collect();
        ContactEmail::delete_by_ids(ids, &self.0).await?;
        Ok(())
    }
}

impl From<ContactEmail> for contact_database::ContactEmail {
    fn from(value: ContactEmail) -> Self {
        Self {
            local_id: value.id().into(),
            remote_id: value.remote_id,
            local_contact_id: value.local_contact_id.expect("should be set").into(),
            canonical_email: value.canonical_email,
            contact_type: value.contact_type.into_inner(),
            defaults: value.defaults.into(),
            display_order: value.display_order,
            email: value.email,
            is_proton: value.is_proton,
            label_ids: vec![],
            last_used_time: value.last_used_time.as_u64(),
            name: value.name,
        }
    }
}

impl From<&contact_database::ContactEmail> for ContactEmail {
    fn from(value: &contact_database::ContactEmail) -> Self {
        Self {
            local_id: Some(value.local_id.into()),
            remote_id: value.remote_id.clone(),
            remote_contact_id: None,
            local_contact_id: Some(value.local_contact_id.into()),
            canonical_email: value.canonical_email.clone(),
            contact_type: ContactTypes::new(value.contact_type.clone()),
            defaults: value.defaults.into(),
            display_order: value.display_order,
            email: value.canonical_email.clone(),
            is_proton: value.is_proton,
            label_ids: vec![],
            last_used_time: value.last_used_time.into(),
            name: value.name.clone(),
        }
    }
}

impl From<contact_database::NewContactEmail> for ContactEmail {
    fn from(value: contact_database::NewContactEmail) -> Self {
        Self {
            local_id: None,
            remote_id: None,
            remote_contact_id: None,
            local_contact_id: Some(value.contact_id.into()),
            canonical_email: value.canonical_email,
            contact_type: ContactTypes::new(value.contact_type),
            defaults: value.defaults.into(),
            display_order: value.display_order,
            email: value.email,
            is_proton: value.is_proton,
            label_ids: vec![],
            last_used_time: value.last_used_time.into(),
            name: value.name,
        }
    }
}

impl From<contact_database::UpseratableContactEmail> for ContactEmail {
    fn from(value: contact_database::UpseratableContactEmail) -> Self {
        Self {
            local_id: None,
            remote_id: Some(value.id),
            remote_contact_id: Some(value.contact_id),
            local_contact_id: None,
            canonical_email: value.canonical_email,
            contact_type: ContactTypes::new(value.contact_type),
            defaults: value.defaults.into(),
            display_order: value.display_order,
            email: value.email,
            is_proton: value.is_proton,
            label_ids: vec![],
            last_used_time: value.last_used_time.into(),
            name: value.name,
        }
    }
}

fn find_contact_email_by_id(
    conn: &Connection,
    id: LocalContactEmailId,
) -> Result<Option<contact_database::ContactEmail>, StashError> {
    let Some(contact) = ContactEmail::find_first_raw_sync("WERE local_id =?", [id], conn)? else {
        return Ok(None);
    };

    let mut contact = contact.into();
    load_contact_email_groups(conn, std::slice::from_mut(&mut contact))?;
    Ok(Some(contact))
}

fn find_contact_email_by_remote_id(
    conn: &Connection,
    id: &ContactEmailId,
) -> Result<Option<contact_database::ContactEmail>, StashError> {
    let Some(contact) = ContactEmail::find_first_raw_sync("WERE remote_id =?", [id], conn)? else {
        return Ok(None);
    };

    let mut contact = contact.into();
    load_contact_email_groups(conn, std::slice::from_mut(&mut contact))?;
    Ok(Some(contact))
}

fn count_contact_emails_in_group_with_local_id(
    conn: &Connection,
    id: LocalContactGroupId,
) -> Result<usize, StashError> {
    Ok(conn.query_row("SELECT DISTINCT COUNT(local_contact_email_id) FROM contact_email_groups WHERE local_contact_group_id = ?",
[id], |r|r.get::<usize,usize>(0))?)
}

fn count_contact_emails_in_group_with_name(
    conn: &Connection,
    name: &str,
) -> Result<usize, StashError> {
    Ok(conn.query_row("SELECT DISTINCT COUNT(local_contact_email_id) FROM contact_email_groups WHERE local_contact_group_id = (SELECT local_id FROM contact_group WHERE name =? LIMIT 1)",
[name], |r|r.get::<usize,usize>(0))?)
}

fn count_contact_emails_in_group_with_remote_id(
    conn: &Connection,
    id: &ContactGroupId,
) -> Result<usize, StashError> {
    Ok(conn.query_row("SELECT DISTINCT COUNT(local_contact_email_id) FROM contact_email_groups WHERE local_contact_group_id = (SELECT local_id FROM contact_group WHERE remote_id =? LIMIT 1)",
[id], |r|r.get::<usize,usize>(0))?)
}

fn load_contact_email_groups(
    conn: &Connection,
    contact_emails: &mut [contact_database::ContactEmail],
) -> Result<(), StashError> {
    let mut stmt = conn.prepare_cached(
        "SELECT local_contact_group_id FROM contact_email_groups WHERE local_contact_email_id = ?",
    )?;
    for contact_email in contact_emails {
        let rows = stmt.query_map([contact_email.local_id.as_u64()], |r| {
            r.get::<usize, u64>(0)
                .map(contact_database::LocalContactGroupId::from)
        })?;

        let mut label_ids = Vec::with_capacity(rows.size_hint().0);
        for row in rows {
            label_ids.push(row?);
        }
        contact_email.label_ids = label_ids;
    }

    Ok(())
}
