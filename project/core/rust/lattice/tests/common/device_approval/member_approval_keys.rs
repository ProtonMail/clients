use lattice::auth::LtAuthUserKeyId;
use lattice::core::LtCoreAddressesListRes;
use lattice::core::members::devices::LtCoreResetAuthDevicesUserKey;
use proton_crypto::crypto::{DataEncoding, PGPProviderSync};
use proton_crypto_account::keys::{LockedKey, UnlockedAddressKey, UnlockedUserKey, UserKeys};
use proton_crypto_account::salts::KeySecret;

use super::super::org_members::decrypt_org_armored_token;
use super::super::unprivatize_admin::AdminPgpState;
use super::admin_device_approval_error::AdminDeviceApprovalError;
use super::device_secret::DeviceSecret;
use super::device_secret_error::DeviceSecretError;

pub(crate) struct MemberApprovalKeys<P: PGPProviderSync> {
    pub(crate) user_keys: Vec<UnlockedUserKey<P>>,
    pub(crate) address_keys: Vec<UnlockedAddressKey<P>>,
}

pub(crate) fn unlock_member_approval_keys<P: PGPProviderSync>(
    admin_state: &AdminPgpState<P>,
    addrs: &LtCoreAddressesListRes,
    member_keys: &UserKeys,
    member_org_passphrase: Option<&KeySecret>,
) -> Result<MemberApprovalKeys<P>, AdminDeviceApprovalError> {
    let user_keys = unlock_org_managed_user_keys(admin_state, member_keys, member_org_passphrase)?;
    let address_keys = unlock_member_address_keys(
        &admin_state.pgp,
        addrs,
        &user_keys,
        &admin_state.key_passphrase,
    )?;
    Ok(MemberApprovalKeys {
        user_keys,
        address_keys,
    })
}

pub(crate) fn decryption_keys_for_activation<'a, P: PGPProviderSync>(
    approval_keys: &'a MemberApprovalKeys<P>,
    addrs: &LtCoreAddressesListRes,
    activation_address_id: &str,
) -> Result<Vec<&'a P::PrivateKey>, AdminDeviceApprovalError> {
    let decrypt_keys: Vec<_> = approval_keys
        .address_keys
        .iter()
        .filter(|k| {
            addrs.addresses.iter().any(|a| {
                a.id.0 == activation_address_id && a.keys.0.as_ref().iter().any(|lk| lk.id == k.id)
            })
        })
        .map(|k| &k.private_key)
        .collect();
    if decrypt_keys.is_empty() {
        return Err(AdminDeviceApprovalError::NoDecryptKeysForActivation {
            activation_address_id: activation_address_id.to_string(),
        });
    }
    Ok(decrypt_keys)
}

pub fn device_secret_from_activation<P: PGPProviderSync>(
    pgp: &P,
    decrypt_keys: &[&P::PrivateKey],
    activation_token: &str,
    typed_code: &str,
) -> Result<DeviceSecret, AdminDeviceApprovalError> {
    let secret_b64 =
        DeviceSecret::decrypt_activation_token_armored(pgp, decrypt_keys, activation_token)
            .map_err(AdminDeviceApprovalError::Crypto)?;
    let secret_bytes = data_encoding::BASE64
        .decode(secret_b64.as_bytes())
        .map_err(DeviceSecretError::Base64Decode)
        .map_err(AdminDeviceApprovalError::Crypto)?;
    if secret_bytes.len() != 32 {
        return Err(AdminDeviceApprovalError::Crypto(
            DeviceSecretError::InvalidSecretLength {
                expected: 32,
                actual: secret_bytes.len(),
            },
        ));
    }
    let mut bytes = [0u8; 32];
    bytes.copy_from_slice(&secret_bytes);
    let device_secret = DeviceSecret(bytes);

    if typed_code.to_uppercase() != device_secret.display_code() {
        return Err(AdminDeviceApprovalError::InvalidConfirmationCode);
    }
    Ok(device_secret)
}

pub(crate) fn rearmor_user_keys<P: PGPProviderSync>(
    admin_state: &AdminPgpState<P>,
    locked_keys: &[LockedKey],
    unlocked: &[UnlockedUserKey<P>],
    new_passphrase: &KeySecret,
) -> Result<Vec<LtCoreResetAuthDevicesUserKey>, AdminDeviceApprovalError> {
    let mut out = Vec::with_capacity(locked_keys.len());
    for locked in locked_keys {
        let decrypted = unlocked.iter().find(|u| u.id == locked.id).ok_or_else(|| {
            AdminDeviceApprovalError::Unlock(format!("unlocked key {} not found", locked.id))
        })?;
        let armored = admin_state
            .pgp
            .private_key_export(
                &decrypted.private_key,
                new_passphrase.as_ref(),
                DataEncoding::Armor,
            )
            .map_err(|e| AdminDeviceApprovalError::Pgp(e.to_string()))?;
        let private_key = String::from_utf8(armored.as_ref().to_vec())
            .map_err(|e| AdminDeviceApprovalError::Pgp(e.to_string()))?;
        out.push(LtCoreResetAuthDevicesUserKey {
            id: LtAuthUserKeyId(locked.id.0.clone()),
            private_key: lattice::Sensitive::new(private_key),
        });
    }
    Ok(out)
}

fn unlock_org_managed_user_keys<P: PGPProviderSync>(
    admin: &AdminPgpState<P>,
    member_keys: &UserKeys,
    member_org_passphrase: Option<&KeySecret>,
) -> Result<Vec<UnlockedUserKey<P>>, AdminDeviceApprovalError> {
    let mut unlocked = Vec::new();
    for locked in member_keys.as_ref() {
        let passphrase = if let Some(org_token) = locked.activation.as_ref() {
            decrypt_org_armored_token(&admin.pgp, &admin.org_private, org_token, true)
                .map_err(AdminDeviceApprovalError::Unlock)?
        } else if let Some(pass) = member_org_passphrase {
            pass.clone()
        } else {
            return Err(AdminDeviceApprovalError::MissingOrgToken {
                key_id: locked.id.0.clone(),
            });
        };
        let private_key = admin
            .pgp
            .private_key_import(
                locked.private_key.0.as_bytes(),
                passphrase.as_ref(),
                DataEncoding::Armor,
            )
            .map_err(|e| AdminDeviceApprovalError::Pgp(e.to_string()))?;
        let public_key = admin
            .pgp
            .private_key_to_public_key(&private_key)
            .map_err(|e| AdminDeviceApprovalError::Pgp(e.to_string()))?;
        unlocked.push(UnlockedUserKey::<P> {
            id: locked.id.clone(),
            private_key,
            public_key,
        });
    }
    if unlocked.is_empty() {
        return Err(AdminDeviceApprovalError::Unlock(
            "no member user keys unlocked".into(),
        ));
    }
    Ok(unlocked)
}

fn unlock_member_address_keys<P: PGPProviderSync>(
    pgp: &P,
    addrs: &LtCoreAddressesListRes,
    user_keys: &[UnlockedUserKey<P>],
    org_passphrase: &KeySecret,
) -> Result<Vec<UnlockedAddressKey<P>>, AdminDeviceApprovalError> {
    let mut all = Vec::new();
    for address in &addrs.addresses {
        let unlock = address.keys.0.unlock(pgp, user_keys, Some(org_passphrase));
        all.extend(unlock.unlocked_keys);
    }
    Ok(all)
}

#[cfg(test)]
mod tests {
    use core_key::generate_user_and_address_keys;
    use lattice::core::LtCoreAddressFlags;
    use proton_crypto::crypto::{DataEncoding, PGPProviderSync};
    use proton_crypto::new_pgp_provider;
    use proton_crypto_account::keys::KeyId;

    use super::*;
    use crate::common::device_approval::device_secret::DeviceSecret;

    #[test]
    fn device_secret_from_activation_rejects_wrong_confirmation_code() {
        let pgp = new_pgp_provider();
        let password = "test-password";
        let (user_key, addr_keys) = generate_user_and_address_keys(
            password,
            [("addr1", "test@example.com", LtCoreAddressFlags::default())],
        )
        .expect("generate keys for unit test");
        let (_id, addr_key) = &addr_keys[0];

        let unlocked_user = user_key
            .key
            .unlock_and_assign_key_id(&pgp, KeyId(String::new()), &user_key.pass)
            .expect("unlock user key");
        let unlocked_addr = addr_key
            .key
            .unlock_and_assign_key_id(&pgp, KeyId(String::new()), &unlocked_user)
            .expect("unlock address key");

        let public_key = pgp
            .private_key_to_public_key(&unlocked_addr.private_key)
            .expect("derive public key");
        let public_armor = pgp
            .public_key_export(&public_key, DataEncoding::Armor)
            .expect("export public key");
        let public_armor =
            String::from_utf8(public_armor.as_ref().to_vec()).expect("armored public key is utf8");

        let device_secret = DeviceSecret::random();
        let activation_token = device_secret
            .encrypt_activation_token(&pgp, &public_armor)
            .expect("encrypt activation token");

        let err = device_secret_from_activation(
            &pgp,
            &[&unlocked_addr.private_key],
            &activation_token,
            "WRONG",
        )
        .expect_err("wrong confirmation code");

        assert!(matches!(
            err,
            AdminDeviceApprovalError::InvalidConfirmationCode
        ));
    }
}
