use crate::fburl::FbUrl;
use crate::gender::{Gender, GenderValue};
use crate::geo::Geo;
use crate::impp::Impp;
use crate::kind::{Kind, KindValue};
use crate::nickname::Nickname;
use crate::organization::Organization;
use crate::photo::Photo;
use crate::revision::Revision;
use crate::sound::Sound;
use crate::source::Source;
use crate::telephone::{Telephone, TelephoneValue};
use crate::time_zone::TimeZoneValue;
use crate::values::component::Component;
use crate::values::iana_token::IanaToken;
use crate::values::text_list::TextList;
use crate::values::timestamp::Timestamp;
use crate::values::uri::Uri;
use crate::values::x_name::XName;

#[test]
fn fburl_struct() {
    let fb_url = FbUrl::new_validated("uri:uri").unwrap();
    assert_eq!(fb_url.value, Uri::new("uri:uri".parse().unwrap()));
}

#[test]
fn gender_struct() {
    let gender = Gender::new_validated("").unwrap();
    assert_eq!(gender.value, GenderValue::None(String::new()));
    let gender = Gender::new_validated("m").unwrap();
    assert_eq!(gender.value, GenderValue::Male(String::new()));
    let gender = Gender::new_validated("M").unwrap();
    assert_eq!(gender.value, GenderValue::Male(String::new()));
    let gender = Gender::new_validated("f").unwrap();
    assert_eq!(gender.value, GenderValue::Female(String::new()));
    let gender = Gender::new_validated("F").unwrap();
    assert_eq!(gender.value, GenderValue::Female(String::new()));
    let gender = Gender::new_validated("o").unwrap();
    assert_eq!(gender.value, GenderValue::Other(String::new()));
    let gender = Gender::new_validated("O").unwrap();
    assert_eq!(gender.value, GenderValue::Other(String::new()));
    let gender = Gender::new_validated("n").unwrap();
    assert_eq!(gender.value, GenderValue::NotApplicable(String::new()));
    let gender = Gender::new_validated("N").unwrap();
    assert_eq!(gender.value, GenderValue::NotApplicable(String::new()));
    let gender = Gender::new_validated("u").unwrap();
    assert_eq!(gender.value, GenderValue::Unknown(String::new()));
    let gender = Gender::new_validated("U").unwrap();
    assert_eq!(gender.value, GenderValue::Unknown(String::new()));
    let gender = Gender::new_validated(";it's complicated").unwrap();
    assert_eq!(
        gender.value,
        GenderValue::None("it's complicated".to_owned())
    );
}

#[test]
fn geo_struct() {
    let geo = Geo::new_validated("uri:uri").unwrap();
    assert_eq!(geo.value, Uri::new_validated("uri:uri").unwrap());
}

#[test]
fn impp_struct() {
    let impp = Impp::new_validated("uri:uri").unwrap();
    assert_eq!(impp.value, Uri::new_validated("uri:uri").unwrap());
}

#[test]
fn kind_struct() {
    let kind = Kind::new_validated("iNdIvIdUaL").unwrap();
    assert_eq!(kind.value, KindValue::Individual);
    let kind = Kind::new_validated("gRoUp").unwrap();
    assert_eq!(kind.value, KindValue::Group);
    let kind = Kind::new_validated("oRg").unwrap();
    assert_eq!(kind.value, KindValue::Organization);
    let kind = Kind::new_validated("lOcAtIoN").unwrap();
    assert_eq!(kind.value, KindValue::Location);
    let kind = Kind::new_validated("IaNa").unwrap();
    assert_eq!(
        kind.value,
        KindValue::IanaToken(IanaToken::new_unchecked("IaNa"))
    );
    let kind = Kind::new_validated("X-NaMe").unwrap();
    assert_eq!(kind.value, KindValue::XName(XName::new_unchecked("X-NaMe")));
}

#[test]
fn nickname_struct() {
    let nickname = Nickname {
        value: "a,b,c".into(),
        ..Default::default()
    };
    assert_eq!(nickname.value, TextList::from("a,b,c"));
}

#[test]
fn organization_struct() {
    let organization = Organization::new_validated("a;b").unwrap();
    assert_eq!(organization.values.len(), 2);
    assert_eq!(organization.values[0], Component::new("a"));
    assert_eq!(organization.values[1], Component::new("b"));
}

#[test]
fn photo_struct() {
    let photo = Photo::new_validated("uri:uri").unwrap();
    assert_eq!(photo.value, Uri::new_validated("uri:uri").unwrap());
}

#[test]
fn revision_struct() {
    let revision = Revision::new_validated("99991231T235959+2359").unwrap();
    assert_eq!(
        revision.value,
        Timestamp::new_validated("99991231T235959+2359").unwrap()
    );
}

#[test]
fn sound_struct() {
    let sound = Sound::new_validated("uri:uri").unwrap();
    assert_eq!(sound.value, Uri::new_validated("uri:uri").unwrap());
}

#[test]
fn source_struct() {
    let source = Source::new_validated("uri:uri").unwrap();
    assert_eq!(source.value, Uri::new_validated("uri:uri").unwrap());
}

#[test]
fn telephone_struct() {
    let telephone = Telephone::new("text".to_string());
    assert_eq!(telephone.value, TelephoneValue::Text("text".to_string()));
    let telephone = Telephone::new("uri:uri".to_string());
    assert_eq!(
        telephone.value,
        TelephoneValue::Uri("uri:uri".parse().unwrap())
    );
}

#[test]
fn time_zone_struct() {
    let tz_text = TimeZoneValue::from("text");
    let tz_uri = TimeZoneValue::from("uri:uri");
    let tz_tz = TimeZoneValue::from("+0130");

    assert!(matches!(tz_text, TimeZoneValue::Text(_)));
    assert!(matches!(tz_uri, TimeZoneValue::Uri(_)));
    assert!(matches!(tz_tz, TimeZoneValue::UtcOffset(_)));
}
