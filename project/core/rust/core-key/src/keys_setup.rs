use crate::{NewAddrKey, NewUserKey, SharedCryptoError, new_key_flags};
use lattice::Sensitive;
use lattice::core::LtCoreAddressFlags;
use lattice::core::LtCoreAddressKeyInput;
use lattice::core::keys::LtCoreSetupKeysBody;
use lattice::core::user::LtCoreSrpVerifier;
use proton_crypto_account::proton_crypto::{new_pgp_provider, new_srp_provider};

pub fn generate_user_and_address_keys<'a, I>(
    password: &str,
    addresses: I,
) -> Result<(NewUserKey, Vec<(String, NewAddrKey)>), SharedCryptoError>
where
    I: IntoIterator<Item = (&'a str, &'a str, LtCoreAddressFlags)>,
{
    let srp = new_srp_provider();
    let pgp = new_pgp_provider();

    let user_key = NewUserKey::init(&srp, &pgp, password)?;

    let addr_keys = addresses
        .into_iter()
        .map(|(id, email, flags)| {
            let addr_key = user_key.init_addr_key(&pgp, email, new_key_flags(flags))?;
            Ok((id.to_owned(), addr_key))
        })
        .collect::<Result<Vec<_>, SharedCryptoError>>()?;

    Ok((user_key, addr_keys))
}

pub fn build_setup_keys_body<'a, I>(
    auth: LtCoreSrpVerifier,
    user_key: &NewUserKey,
    addr_keys: I,
    encrypted_secret: Option<Sensitive<String>>,
    org_primary_user_key: Option<Sensitive<String>>,
    org_activation_token: Option<Sensitive<String>>,
) -> LtCoreSetupKeysBody
where
    I: IntoIterator<Item = (&'a str, &'a NewAddrKey)>,
{
    let address_keys = addr_keys
        .into_iter()
        .map(|(id, key)| LtCoreAddressKeyInput {
            address_id: id.to_owned(),
            private_key: Sensitive::new(key.key.private_key.to_string()),
            token: key
                .key
                .token
                .as_ref()
                .map(|t| Sensitive::new(t.to_string())),
            signature: key
                .key
                .signature
                .as_ref()
                .map(|t| Sensitive::new(t.to_string())),
            signed_key_list: key.skl.clone().into(),
            revision: 0,
            primary: 1,
        })
        .collect();

    LtCoreSetupKeysBody {
        auth,
        primary_key: Sensitive::new(user_key.key.private_key.to_string()),
        key_salt: Sensitive::new(user_key.salt.to_string()),
        address_keys,
        encrypted_secret,
        org_primary_user_key,
        org_activation_token,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lattice::core::LtCoreAddressFlags;

    #[test]
    fn build_setup_keys_body_maps_fields_and_sso_extras() {
        let flags = LtCoreAddressFlags::default();
        let addresses = [
            ("addr-1", "alice@example.com", flags),
            ("addr-2", "bob@example.com", flags),
        ];
        let (user_key, addr_keys) = generate_user_and_address_keys(
            "hunter2",
            addresses
                .iter()
                .map(|(id, email, flags)| (*id, *email, *flags)),
        )
        .expect("generate keys");

        assert_eq!(addr_keys.len(), 2);
        assert_eq!(addr_keys[0].0, "addr-1");
        assert_eq!(addr_keys[1].0, "addr-2");

        let auth = LtCoreSrpVerifier {
            version: 4,
            modulus_id: "modulus-id".to_string(),
            salt: Sensitive::new("srp-salt".to_string()),
            verifier: Sensitive::new("srp-verifier".to_string()),
        };

        let body = build_setup_keys_body(
            auth.clone(),
            &user_key,
            addr_keys.iter().map(|(id, k)| (id.as_str(), k)),
            Some(Sensitive::new("encrypted-secret".to_string())),
            Some(Sensitive::new("org-primary-user-key".to_string())),
            Some(Sensitive::new("org-activation-token".to_string())),
        );

        assert_eq!(body.auth, auth);
        assert_eq!(
            body.primary_key.as_str(),
            user_key.key.private_key.to_string().as_str()
        );
        assert_eq!(body.key_salt.as_str(), user_key.salt.to_string().as_str());

        assert_eq!(body.address_keys.len(), 2);
        assert_eq!(body.address_keys[0].address_id, "addr-1");
        assert_eq!(body.address_keys[1].address_id, "addr-2");
        for k in &body.address_keys {
            assert_eq!(k.primary, 1);
            assert_eq!(k.revision, 0);
        }

        assert_eq!(
            body.encrypted_secret.as_ref().map(|s| s.as_str()),
            Some("encrypted-secret")
        );
        assert_eq!(
            body.org_primary_user_key.as_ref().map(|s| s.as_str()),
            Some("org-primary-user-key")
        );
        assert_eq!(
            body.org_activation_token.as_ref().map(|s| s.as_str()),
            Some("org-activation-token")
        );
    }
}
