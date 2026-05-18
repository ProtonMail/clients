use std::io::BufReader;

use base64::Engine as _;
use base64::prelude::BASE64_STANDARD as BASE_64;
use ical::VcardParser;
use mail_vcard::parameters::preference::Preference;
use mail_vcard::vcard::VCard;
use proton_crypto_account::contacts::DecryptableVerifiableCard;
use proton_crypto_account::keys::{EmailMimeType, PGPScheme, PinnedPublicKeys, UnlockedUserKeys};
use proton_crypto_account::proton_crypto::crypto::{DataEncoding, PGPProviderSync, PublicKey};
use tracing::error;

use crate::ContactKeyExtractionError;

const X_PM_ENCRYPT: &str = "X-PM-ENCRYPT";
const X_PM_ENCRYPT_UNTRUSTED: &str = "X-PM-ENCRYPT-UNTRUSTED";
const X_PM_SCHEME: &str = "X-PM-SCHEME";
const X_PM_SIGN: &str = "X-PM-SIGN";
const X_PM_MIMETYPE: &str = "X-PM-MIMETYPE";

/// Extract pinned keys from a contact card.
pub fn extract_pinned_keys<P, C>(
    pgp: &P,
    unlocked_user_keys: &UnlockedUserKeys<P>,
    signed_card_opt: Option<C>,
    email: &str,
) -> crate::Result<Option<PinnedPublicKeys<<P>::PublicKey>>>
where
    P: PGPProviderSync,
    C: DecryptableVerifiableCard,
{
    let Some(signed_card) = signed_card_opt else {
        return Ok(None);
    };

    // Verify the signature of the v-card.
    let card_data =
        signed_card.decrypt_and_verify_sync(pgp, &pgp.empty_private_keys(), unlocked_user_keys)?;

    // Parse the v-card contact, there should be exactly one v-card
    let vcard_contact = VcardParser::new(BufReader::new(card_data.as_slice()))
        .next()
        .ok_or(ContactKeyExtractionError::NoVCard)??;

    let vcard = VCard::try_from(vcard_contact)?;

    Ok(pinned_keys_for_email_address(pgp, &vcard, email))
}

/// Returns all pinned keys for this v-card contact matching the provided email address.
///
/// The email comparison ignores case-sensitivity.
///
/// If no crypto information for this email is found in the vcard, the method returns [`None`].
pub fn pinned_keys_for_email_address<P>(
    pgp: &P,
    vcard: &VCard,
    email: &str,
) -> Option<PinnedPublicKeys<<P>::PublicKey>>
where
    P: PGPProviderSync,
{
    let email_to_check = email.to_lowercase();
    let group = vcard
        .get_all_email()
        .into_iter()
        .map(|(_, email_type)| (email_type.group, email_type.value.value.to_lowercase()))
        .find(|(_, email_lowercase)| email_lowercase == &email_to_check)?
        .0?;

    let mut pinned_keys = pinned_keys_for_group(vcard, pgp, &group);
    update_pinned_keys_with_extended_preferences(vcard, &group, &mut pinned_keys);
    Some(pinned_keys)
}

/// Collect all the keys for the `selected_group` and return them in order of preference.
/// A lower value for preference indicates a higher priority for that key
fn pinned_keys_for_group<P>(
    vcard: &VCard,
    pgp: &P,
    selected_group: &str,
) -> PinnedPublicKeys<<P>::PublicKey>
where
    P: PGPProviderSync,
{
    let mut preference_keys: Vec<_> = vcard
        .get_all_key()
        .into_iter()
        .filter_map(|(_, key)| {
            let group_name = key.group?;
            if group_name == selected_group {
                let key_data = key.value.to_string();

                let pref = unwrap_preference(key.preference);
                let public_key_res = parse_and_import_pgp_key(pgp, &key_data);

                match public_key_res {
                    Err(e) => {
                        error!("error parsing and importing pgp key with error: {:?}", e);
                        return None;
                    }
                    Ok(public_key) => return Some((pref, public_key?)),
                }
            }
            None
        })
        .collect();

    preference_keys.sort_by(|a, b| a.0.cmp(&b.0));
    let mut pinned_keys = Vec::with_capacity(preference_keys.len());
    pinned_keys.extend(preference_keys.into_iter().map(|val| val.1));
    PinnedPublicKeys::new(pinned_keys)
}

/// Updates the pinned public key preferences in `pinned_keys`
/// based on the preferences found in the matching selected group.
fn update_pinned_keys_with_extended_preferences<Pub: PublicKey>(
    vcard: &VCard,
    selected_group: &str,
    pinned_keys: &mut PinnedPublicKeys<Pub>,
) {
    vcard
        .get_all_xtended()
        .into_iter()
        .filter_map(|(_, property)| {
            if property.group.as_ref()? == selected_group {
                Some(property)
            } else {
                None
            }
        })
        .for_each(|extended_property| {
            let name = extended_property.name.0.as_str();
            let value = extended_property.value.as_deref();

            match name {
                X_PM_ENCRYPT => {
                    pinned_keys.encrypt_to_pinned = parse_bool(value);
                }
                X_PM_ENCRYPT_UNTRUSTED => {
                    pinned_keys.encrypt_to_untrusted = parse_bool(value);
                }
                X_PM_SIGN => {
                    pinned_keys.sign = parse_bool(value);
                }
                X_PM_SCHEME => {
                    pinned_keys.scheme = parse_pgp_scheme(value);
                }
                X_PM_MIMETYPE => {
                    pinned_keys.mime_type = parse_mime_type(value);
                }
                _ => (),
            }
        });
}

fn unwrap_preference(preference: Option<Preference>) -> Preference {
    if let Some(unwrapped) = preference {
        if unwrapped.is_valid_value() {
            unwrapped
        } else {
            Preference::less_than_lowest()
        }
    } else {
        Preference::less_than_lowest()
    }
}

fn parse_bool(value: Option<&str>) -> Option<bool> {
    value
        .as_ref()
        .and_then(|str| str.to_lowercase().parse::<bool>().ok())
}

fn parse_pgp_scheme(value: Option<&str>) -> Option<PGPScheme> {
    value.as_ref().and_then(|str| str.parse().ok())
}

fn parse_mime_type(value: Option<&str>) -> Option<EmailMimeType> {
    value.as_ref().and_then(|str| str.parse().ok())
}

fn parse_and_import_pgp_key<P>(pgp: &P, value: &str) -> crate::Result<Option<<P>::PublicKey>>
where
    P: PGPProviderSync,
{
    let base64_encoded_key = value
        .split(',')
        .next_back()
        .ok_or(ContactKeyExtractionError::NoData)?;

    let binary_key = BASE_64.decode(base64_encoded_key)?;

    Ok(Some(
        pgp.public_key_import(binary_key, DataEncoding::Bytes)?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use proton_crypto_account::proton_crypto::crypto::{
        AccessKeyInfo, AsPublicKeyRef, OpenPGPFingerprint, OpenPGPKeyID, SHA256Fingerprint,
        UnixTimestamp,
    };

    #[derive(Clone, Copy, Debug, Default)]
    struct FakePublicKey;

    impl PublicKey for FakePublicKey {
        //
    }

    impl AccessKeyInfo for FakePublicKey {
        fn version(&self) -> u8 {
            todo!()
        }

        fn key_id(&self) -> OpenPGPKeyID {
            todo!()
        }

        fn key_fingerprint(&self) -> OpenPGPFingerprint {
            todo!()
        }

        fn sha256_key_fingerprints(&self) -> Vec<SHA256Fingerprint> {
            todo!()
        }

        fn can_encrypt(&self, _: UnixTimestamp) -> bool {
            todo!()
        }

        fn can_verify(&self, _: UnixTimestamp) -> bool {
            todo!()
        }

        fn is_expired(&self, _: UnixTimestamp) -> bool {
            todo!()
        }

        fn is_revoked(&self, _: UnixTimestamp) -> bool {
            todo!()
        }
    }

    impl AsPublicKeyRef<FakePublicKey> for FakePublicKey {
        fn as_public_key(&self) -> &FakePublicKey {
            self
        }
    }

    mod extended_preferences {
        use super::*;
        use mail_vcard::parameters::Parameters;
        use mail_vcard::xtended::Xtended;
        use pretty_assertions as pa;
        use test_case::test_case;

        struct TestCase {
            given_prefs: &'static [(&'static str, &'static str)],
            expected: fn() -> PinnedPublicKeys<FakePublicKey>,
        }

        const TEST_X_PM_ENCRYPT_FALSE: TestCase = TestCase {
            given_prefs: &[(X_PM_ENCRYPT, "false")],
            expected: || PinnedPublicKeys {
                encrypt_to_pinned: Some(false),
                ..PinnedPublicKeys::default()
            },
        };

        const TEST_X_PM_ENCRYPT_TRUE: TestCase = TestCase {
            given_prefs: &[(X_PM_ENCRYPT, "true")],
            expected: || PinnedPublicKeys {
                encrypt_to_pinned: Some(true),
                ..PinnedPublicKeys::default()
            },
        };

        const TEST_X_PM_ENCRYPT_UNTRUSTED_FALSE: TestCase = TestCase {
            given_prefs: &[(X_PM_ENCRYPT_UNTRUSTED, "false")],
            expected: || PinnedPublicKeys {
                encrypt_to_untrusted: Some(false),
                ..PinnedPublicKeys::default()
            },
        };

        const TEST_X_PM_ENCRYPT_UNTRUSTED_TRUE: TestCase = TestCase {
            given_prefs: &[(X_PM_ENCRYPT_UNTRUSTED, "true")],
            expected: || PinnedPublicKeys {
                encrypt_to_untrusted: Some(true),
                ..PinnedPublicKeys::default()
            },
        };

        const TEST_X_PM_SIGN_TRUE: TestCase = TestCase {
            given_prefs: &[(X_PM_SIGN, "true")],
            expected: || PinnedPublicKeys {
                sign: Some(true),
                ..PinnedPublicKeys::default()
            },
        };

        const TEST_X_PM_SIGN_FALSE: TestCase = TestCase {
            given_prefs: &[(X_PM_SIGN, "false")],
            expected: || PinnedPublicKeys {
                sign: Some(false),
                ..PinnedPublicKeys::default()
            },
        };

        const TEST_X_PM_SCHEME_INLINE: TestCase = TestCase {
            given_prefs: &[(X_PM_SCHEME, "pgp-inline")],
            expected: || PinnedPublicKeys {
                scheme: Some(PGPScheme::PGPInline),
                ..PinnedPublicKeys::default()
            },
        };

        const TEST_X_PM_SCHEME_MIME: TestCase = TestCase {
            given_prefs: &[(X_PM_SCHEME, "pgp-mime")],
            expected: || PinnedPublicKeys {
                scheme: Some(PGPScheme::PGPMime),
                ..PinnedPublicKeys::default()
            },
        };

        const TEST_X_PM_MIMETYPE_EMPTY: TestCase = TestCase {
            given_prefs: &[(X_PM_MIMETYPE, "")],
            expected: || PinnedPublicKeys {
                mime_type: None,
                ..PinnedPublicKeys::default()
            },
        };

        const TEST_X_PM_MIMETYPE_TEXT_PLAIN: TestCase = TestCase {
            given_prefs: &[(X_PM_MIMETYPE, "text/plain")],
            expected: || PinnedPublicKeys {
                mime_type: Some(EmailMimeType::Text),
                ..PinnedPublicKeys::default()
            },
        };

        #[allow(clippy::needless_pass_by_value)]
        #[test_case(TEST_X_PM_ENCRYPT_FALSE)]
        #[test_case(TEST_X_PM_ENCRYPT_TRUE)]
        #[test_case(TEST_X_PM_ENCRYPT_UNTRUSTED_FALSE)]
        #[test_case(TEST_X_PM_ENCRYPT_UNTRUSTED_TRUE)]
        #[test_case(TEST_X_PM_SIGN_FALSE)]
        #[test_case(TEST_X_PM_SIGN_TRUE)]
        #[test_case(TEST_X_PM_SCHEME_INLINE)]
        #[test_case(TEST_X_PM_SCHEME_MIME)]
        #[test_case(TEST_X_PM_MIMETYPE_EMPTY)]
        #[test_case(TEST_X_PM_MIMETYPE_TEXT_PLAIN)]
        fn test(case: TestCase) {
            let vcard = {
                let mut vcard = VCard::default();

                for &(name, value) in case.given_prefs {
                    vcard
                        .add_xtended(Xtended {
                            name: name.try_into().unwrap(),
                            value: Some(value.into()),
                            parameters: Parameters::default(),
                            group: Some("group".into()),
                        })
                        .unwrap();
                }

                vcard
            };

            let mut target = PinnedPublicKeys::default();

            update_pinned_keys_with_extended_preferences::<FakePublicKey>(
                &vcard,
                "group",
                &mut target,
            );

            // PinnedPublicKeys are `!PartialEq`, so comparing them via the
            // debug impl is the best we can do
            pa::assert_eq!(format!("{:#?}", (case.expected)()), format!("{target:#?}"));
        }
    }
}
