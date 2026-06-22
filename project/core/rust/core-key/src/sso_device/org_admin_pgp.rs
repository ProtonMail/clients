use lattice::Sensitive;
use lattice::auth::devices::LtAuthDevice;
use lattice::auth::{LtAuthAddressId, LtAuthUserKeyId};
use lattice::core::{
    LtCoreAddress, LtCoreAuthDeviceId, LtCorePostMembersDevicesResetBody,
    LtCoreResetAuthDevicesUserKey, LtCoreUnprivActivationToken,
};
use proton_crypto::crypto::{
    AsPublicKeyRef, DataEncoding, Decryptor, DecryptorSync, DetachedSignatureVariant, Encryptor,
    EncryptorSync, PGPProviderSync, VerifiedData,
};
use proton_crypto_account::keys::{
    ArmoredPrivateKey, EncryptedKeyToken, KeyId, KeyTokenSignature, LocalUserKey, LockedKey,
    UnlockedAddressKeys, UnlockedUserKey, UnlockedUserKeys, UserKeys,
};
use proton_crypto_account::salts::KeySecret;

use crate::sso_device::device_secret::DeviceSecret;
use crate::sso_device::encrypted_secret::EncryptedSecret;
use crate::sso_device::secure_hex_key_secret_32;
use crate::{DeviceDisplayCode, SharedCryptoError};

pub struct OrgAdminPgp<'a, P: PGPProviderSync> {
    pub pgp: &'a P,
    pub(crate) org_private: &'a P::PrivateKey,
    pub(crate) admin_key_passphrase: &'a KeySecret,
}

impl<'a, P: PGPProviderSync> OrgAdminPgp<'a, P> {
    pub fn new(
        pgp: &'a P,
        org_private: &'a P::PrivateKey,
        admin_key_passphrase: &'a KeySecret,
    ) -> Self {
        Self {
            pgp,
            org_private,
            admin_key_passphrase,
        }
    }

    pub(crate) fn public_key(&self) -> Result<P::PublicKey, SharedCryptoError> {
        self.pgp
            .private_key_to_public_key(self.org_private)
            .map_err(SharedCryptoError::from)
    }

    pub(crate) fn decrypt(
        &self,
        bytes: &[u8],
        verify: bool,
        encoding: DataEncoding,
    ) -> Result<Sensitive<Vec<u8>>, SharedCryptoError> {
        if verify {
            let org_public = self.public_key()?;
            let verified = self
                .pgp
                .new_decryptor()
                .with_decryption_key(self.org_private)
                .with_verification_key_refs(&[&org_public])
                .decrypt(bytes, encoding)
                .map_err(SharedCryptoError::from)?;
            Ok(Sensitive::new(verified.to_vec()))
        } else {
            let verified = self
                .pgp
                .new_decryptor()
                .with_decryption_key(self.org_private)
                .decrypt(bytes, encoding)
                .map_err(SharedCryptoError::from)?;
            Ok(Sensitive::new(verified.to_vec()))
        }
    }

    pub(crate) fn decrypt_armored(
        &self,
        bytes: &[u8],
        verify: bool,
    ) -> Result<Sensitive<Vec<u8>>, SharedCryptoError> {
        self.decrypt(bytes, verify, DataEncoding::Armor)
    }

    /// Decrypt an org-armored passphrase blob.
    ///
    /// - `verify: true` — org-signed key tokens on locked user keys (`LockedKey.activation`).
    /// - `verify: false` — unprivatization list `activation_token` (encrypt-only, not signed).
    pub(crate) fn decrypt_org_armored_token(
        &self,
        activation_token_armored: &LtCoreUnprivActivationToken,
        verify: bool,
    ) -> Result<KeySecret, SharedCryptoError> {
        self.decrypt_armored(activation_token_armored.as_ref(), verify)
            .map(|v| KeySecret::new(v.into_inner()))
    }

    pub fn encrypt(
        &self,
        bytes: &[u8],
        encoding: DataEncoding,
    ) -> Result<Sensitive<Vec<u8>>, SharedCryptoError> {
        let org_public = self.public_key()?;
        let encrypted = self
            .pgp
            .new_encryptor()
            .with_encryption_key(&org_public)
            .encrypt_raw(bytes, encoding)?;
        Ok(Sensitive::new(encrypted))
    }

    /// Encrypt a secret for storage on member user keys (org public key).
    pub fn encrypt_org_armored_token(
        &self,
        secret: &KeySecret,
    ) -> Result<String, SharedCryptoError> {
        let encrypted = self.encrypt(secret.as_ref(), DataEncoding::Armor)?;
        Ok(String::from_utf8(encrypted.into_inner())?)
    }

    fn unlock_passphrase_for_locked(
        &self,
        locked: &LockedKey,
        member_org_passphrase: Option<&KeySecret>,
    ) -> Result<KeySecret, SharedCryptoError> {
        if let Some(activation) = &locked.activation {
            let activation = LtCoreUnprivActivationToken(Sensitive::new(activation.clone()));
            self.decrypt_org_armored_token(&activation, true)
        } else if let Some(pass) = member_org_passphrase {
            Ok(pass.clone())
        } else {
            Err(SharedCryptoError::MissingOrgToken {
                key_id: locked.id.0.clone(),
            })
        }
    }

    pub fn rearmor_user_keys(
        &self,
        unlocked: &UnlockedUserKeys<P>,
        new_passphrase: &KeySecret,
    ) -> Result<Vec<LtCoreResetAuthDevicesUserKey>, SharedCryptoError> {
        unlocked
            .iter()
            .map(|decrypted| {
                let local = LocalUserKey::relock_user_key(self.pgp, decrypted, new_passphrase)?;
                Ok(LtCoreResetAuthDevicesUserKey {
                    id: LtAuthUserKeyId(decrypted.id.0.clone()),
                    private_key: Sensitive::new(local.private_key.0.clone()),
                })
            })
            .collect()
    }

    pub fn collect_decrypt_keys_for_activation_address<'b>(
        &self,
        address_keys: &'b UnlockedAddressKeys<P>,
        addrs: &[LtCoreAddress],
        activation_address_id: &LtAuthAddressId,
    ) -> Result<Vec<&'b P::PrivateKey>, SharedCryptoError> {
        let decrypt_keys: Vec<_> = address_keys
            .iter()
            .filter(|k| {
                addrs.iter().any(|a| {
                    &a.id == activation_address_id
                        && a.keys.0.as_ref().iter().any(|lk| lk.id == k.id)
                })
            })
            .map(|k| &k.private_key)
            .collect();
        if decrypt_keys.is_empty() {
            return Err(SharedCryptoError::NoDecryptKeysForActivation {
                activation_address_id: activation_address_id.to_string(),
            });
        }
        Ok(decrypt_keys)
    }

    /// Unlock all address keys for a member using the org passphrase.
    pub fn unlock_member_address_keys(
        &self,
        addrs: &[LtCoreAddress],
        user_keys: &UnlockedUserKeys<P>,
        key_passphrase: &KeySecret,
    ) -> Result<UnlockedAddressKeys<P>, SharedCryptoError> {
        addrs
            .iter()
            .map(|addr| {
                let unlock = addr
                    .keys
                    .0
                    .unlock(self.pgp, user_keys, Some(key_passphrase));
                if !unlock.failed.is_empty() {
                    return Err(SharedCryptoError::AddressKeysUnlockFailed {
                        failed: unlock.failed,
                    });
                }
                Ok(unlock.unlocked_keys)
            })
            .collect::<Result<Vec<_>, _>>()
            .map(|v| UnlockedAddressKeys::from(v.concat()))
    }

    /// Crypto payload for `POST .../members/{id}/devices/reset`.
    ///
    /// The member's user keys are rotated: re-armored under `new_passphrase`. The
    /// `EncryptedSecret` wraps that same `new_passphrase` (not the member's current
    /// passphrase) under the device key, because that is the value the member's new
    /// device recovers and uses to unlock the freshly re-armored keys. Binding the
    /// current passphrase here would leave the new device unable to unlock the keys
    /// the server now stores under `new_passphrase`.
    pub fn build_devices_reset_crypto(
        &self,
        unlocked_user_keys: &UnlockedUserKeys<P>,
        device_secret: &DeviceSecret,
        new_passphrase: &KeySecret,
        auth_device_id: LtCoreAuthDeviceId,
    ) -> Result<LtCorePostMembersDevicesResetBody, SharedCryptoError> {
        let rearmored_user_keys = self.rearmor_user_keys(unlocked_user_keys, new_passphrase)?;
        let encrypted_secret = EncryptedSecret::from_key_secret(new_passphrase, &device_secret.0)?;

        Ok(LtCorePostMembersDevicesResetBody {
            auth_device_id,
            encrypted_secret: Sensitive::new(encrypted_secret.as_str().to_string()),
            user_keys: rearmored_user_keys,
        })
    }

    /// Crypto payload for admin approval of a pending auth device (`POST .../devices/reset`).
    pub fn build_devices_reset_for_pending(
        &self,
        member_keys: &UserKeys,
        member_addresses: &[LtCoreAddress],
        member_org_passphrase: Option<&KeySecret>,
        pending: &LtAuthDevice,
        typed_code: &str,
    ) -> Result<LtCorePostMembersDevicesResetBody, SharedCryptoError> {
        let typed_code = DeviceDisplayCode::parse(typed_code)?;

        let activation_token = pending.activation_token.as_deref().ok_or(
            SharedCryptoError::PendingAuthDeviceMissingField {
                field: "activation_token",
            },
        )?;
        let activation_address_id = pending.activation_address_id.as_ref().ok_or(
            SharedCryptoError::PendingAuthDeviceMissingField {
                field: "activation_address_id",
            },
        )?;
        let unlocked_user_keys =
            self.unlock_org_managed_user_keys(member_keys, member_org_passphrase)?;
        let unlocked_address_keys = self.unlock_member_address_keys(
            member_addresses,
            &unlocked_user_keys,
            self.admin_key_passphrase,
        )?;
        let decrypt_keys = self.collect_decrypt_keys_for_activation_address(
            &unlocked_address_keys,
            member_addresses,
            activation_address_id,
        )?;
        let device_secret =
            DeviceSecret::from_activation(self.pgp, &decrypt_keys, activation_token, &typed_code)?;
        self.build_devices_reset_crypto(
            &unlocked_user_keys,
            &device_secret,
            &secure_hex_key_secret_32(),
            pending.id.clone(),
        )
    }

    pub fn unlock_org_managed_user_keys(
        &self,
        member_keys: &UserKeys,
        member_org_passphrase: Option<&KeySecret>,
    ) -> Result<UnlockedUserKeys<P>, SharedCryptoError> {
        let unlocked: Vec<UnlockedUserKey<P>> = member_keys
            .as_ref()
            .iter()
            .map(|locked| {
                let passphrase =
                    self.unlock_passphrase_for_locked(locked, member_org_passphrase)?;
                self.unlock_user_key_from_armored(
                    &locked.private_key,
                    &passphrase,
                    locked.id.clone(),
                )
            })
            .collect::<Result<_, SharedCryptoError>>()?;
        if unlocked.is_empty() {
            return Err(SharedCryptoError::NoMemberUserKeysUnlocked);
        }
        Ok(UnlockedUserKeys::from(unlocked))
    }

    pub(crate) fn import_armored_private_key(
        &self,
        armored: &ArmoredPrivateKey,
        passphrase: &KeySecret,
    ) -> Result<P::PrivateKey, SharedCryptoError> {
        self.pgp
            .private_key_import(armored.as_bytes(), passphrase.as_ref(), DataEncoding::Armor)
            .map_err(SharedCryptoError::from)
    }

    pub(crate) fn unlock_user_key_from_armored(
        &self,
        armored: &ArmoredPrivateKey,
        passphrase: &KeySecret,
        id: KeyId,
    ) -> Result<UnlockedUserKey<P>, SharedCryptoError> {
        let private_key = self.import_armored_private_key(armored, passphrase)?;
        let public_key = self.pgp.private_key_to_public_key(&private_key)?;
        Ok(UnlockedUserKey::<P> {
            id,
            private_key,
            public_key,
        })
    }

    pub(crate) fn decrypt_signed_armored_token(
        &self,
        token: &EncryptedKeyToken,
        signature: &KeyTokenSignature,
        member_user_keys: &UnlockedUserKeys<P>,
    ) -> Result<Vec<u8>, SharedCryptoError> {
        let decryption_keys: Vec<_> = member_user_keys.iter().map(|k| &k.private_key).collect();
        let verification_keys: Vec<_> =
            member_user_keys.iter().map(|k| k.as_public_key()).collect();

        let verified = self
            .pgp
            .new_decryptor()
            .with_decryption_key_refs(&decryption_keys)
            .with_verification_key_refs(&verification_keys)
            .with_detached_signature_ref(
                signature.0.as_bytes(),
                DetachedSignatureVariant::Plaintext,
                true,
            )
            .decrypt(token.0.as_bytes(), DataEncoding::Armor)?;
        verified.verification_result()?;
        Ok(verified.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::OrgAdminPgp;
    use crate::keys::NewUserKey;
    use crate::sso_device::device_secret::DeviceSecret;
    use crate::sso_device::encrypted_secret::EncryptedSecret;
    use lattice::core::LtCoreAuthDeviceId;
    use proton_crypto::crypto::{DataEncoding, PGPProviderSync};
    use proton_crypto::{new_pgp_provider, new_srp_provider};
    use proton_crypto_account::keys::UnlockedUserKeys;
    use proton_crypto_account::salts::KeySecret;

    /// The org-admin device reset must bind the rotated `new_passphrase` (not the
    /// member's current passphrase) into `EncryptedSecret`: the value the new device
    /// recovers has to unlock the keys the admin re-armored under `new_passphrase`.
    #[test]
    fn devices_reset_binds_new_passphrase_that_unlocks_rearmored_keys() {
        let pgp = new_pgp_provider();
        let srp = new_srp_provider();

        // Member user key, locked under its original passphrase.
        let member_nk = NewUserKey::init(&srp, &pgp, b"member-old-password").expect("member key");
        let old_pass = member_nk.pass.clone();
        let member_unlocked = member_nk.unlock_user_key(&pgp).expect("unlock member key");
        let unlocked_user_keys = UnlockedUserKeys::from(vec![member_unlocked]);

        // Throwaway org key — only `pgp` is exercised by `build_devices_reset_crypto`.
        let org_nk = NewUserKey::init(&srp, &pgp, b"org-password").expect("org key");
        let org_private = org_nk
            .unlock_user_key(&pgp)
            .expect("unlock org key")
            .private_key;
        let admin_key_passphrase = KeySecret::new(b"admin-key-passphrase".to_vec());
        let admin = OrgAdminPgp::new(&pgp, &org_private, &admin_key_passphrase);

        let device_secret = DeviceSecret::from_bytes([7u8; 32]);
        let new_passphrase = KeySecret::new(b"freshly-rotated-passphrase-0123456789".to_vec());

        let body = admin
            .build_devices_reset_crypto(
                &unlocked_user_keys,
                &device_secret,
                &new_passphrase,
                LtCoreAuthDeviceId("device-test".into()),
            )
            .expect("build devices reset crypto");

        // The new device decrypts EncryptedSecret with its device secret and must
        // recover exactly `new_passphrase`.
        let recovered = EncryptedSecret::new(body.encrypted_secret.as_str())
            .decrypt_to_vec(&device_secret.0)
            .expect("decrypt encrypted secret");
        assert_eq!(
            recovered.as_slice(),
            new_passphrase.as_ref(),
            "EncryptedSecret must wrap the rotated new_passphrase"
        );

        // The re-armored keys must open with the recovered passphrase and not the old one.
        assert!(!body.user_keys.is_empty(), "expected re-armored user keys");
        for uk in &body.user_keys {
            pgp.private_key_import(
                uk.private_key.as_bytes(),
                recovered.as_slice(),
                DataEncoding::Armor,
            )
            .expect("recovered passphrase unlocks the re-armored key");
            assert!(
                pgp.private_key_import(
                    uk.private_key.as_bytes(),
                    old_pass.as_ref(),
                    DataEncoding::Armor,
                )
                .is_err(),
                "the member's old passphrase must not unlock the rotated key"
            );
        }
    }
}
