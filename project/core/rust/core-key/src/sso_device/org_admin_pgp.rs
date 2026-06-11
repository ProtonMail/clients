use lattice::Sensitive;
use lattice::auth::devices::LtAuthDevice;
use lattice::auth::{LtAuthAddressId, LtAuthUserKeyId};
use lattice::core::{
    LtCoreAddress, LtCoreAuthDeviceId, LtCorePostMembersDevicesResetBody,
    LtCoreResetAuthDevicesUserKey, LtCoreUnprivActivationToken,
};
use proton_crypto::crypto::{
    DataEncoding, Decryptor, DecryptorSync, Encryptor, EncryptorSync, PGPProviderSync, VerifiedData,
};
use proton_crypto_account::keys::{
    LocalUserKey, LockedKey, UnlockedAddressKey, UnlockedUserKey, UserKeys,
};
use proton_crypto_account::salts::KeySecret;

use crate::keys::LockedKeysExt;
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

    /// Decrypt an org-armored passphrase blob.
    ///
    /// - `verify: true` — org-signed key tokens on locked user keys (`LockedKey.activation`).
    /// - `verify: false` — unprivatization list `activation_token` (encrypt-only, not signed).
    pub(crate) fn decrypt_org_armored_token(
        &self,
        activation_token_armored: &LtCoreUnprivActivationToken,
        verify: bool,
    ) -> Result<KeySecret, SharedCryptoError> {
        if verify {
            let org_public = self.pgp.private_key_to_public_key(self.org_private)?;
            let verified = self
                .pgp
                .new_decryptor()
                .with_decryption_key(self.org_private)
                .with_verification_key_refs(&[&org_public])
                .decrypt(activation_token_armored.as_bytes(), DataEncoding::Armor)?;
            Ok(KeySecret::new(verified.to_vec()))
        } else {
            let verified = self
                .pgp
                .new_decryptor()
                .with_decryption_key(self.org_private)
                .decrypt(activation_token_armored.as_bytes(), DataEncoding::Armor)?;
            Ok(KeySecret::new(verified.to_vec()))
        }
    }

    /// Encrypt a secret for storage on member user keys (org public key).
    pub fn encrypt_org_armored_token(
        &self,
        secret: &KeySecret,
    ) -> Result<String, SharedCryptoError> {
        let org_public = self.pgp.private_key_to_public_key(self.org_private)?;
        let encrypted = self
            .pgp
            .new_encryptor()
            .with_encryption_key(&org_public)
            .encrypt_raw(secret.as_ref(), DataEncoding::Armor)?;
        Ok(String::from_utf8(encrypted)?)
    }

    /// Current passphrase for the member's primary user key (for `EncryptedSecret` on org-admin reset).
    ///
    /// Mirrors per-key unlock in [`Self::unlock_org_managed_user_keys`] for the primary key only.
    pub fn member_primary_unlock_passphrase(
        &self,
        member_keys: &UserKeys,
        member_org_passphrase: Option<&KeySecret>,
    ) -> Result<KeySecret, SharedCryptoError> {
        let primary = member_keys
            .primary_key()
            .ok_or(SharedCryptoError::NoPrimaryUserKey)?;
        self.unlock_passphrase_for_locked(primary, member_org_passphrase)
    }

    fn unlock_passphrase_for_locked(
        &self,
        locked: &LockedKey,
        member_org_passphrase: Option<&KeySecret>,
    ) -> Result<KeySecret, SharedCryptoError> {
        if let Some(org_token) = locked_activation_token(locked) {
            self.decrypt_org_armored_token(&org_token, true)
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
        locked_keys: &[LockedKey],
        unlocked: &[UnlockedUserKey<P>],
        new_passphrase: &KeySecret,
    ) -> Result<Vec<LtCoreResetAuthDevicesUserKey>, SharedCryptoError> {
        locked_keys
            .iter()
            .map(|locked| {
                let decrypted = unlocked.iter().find(|u| u.id == locked.id).ok_or(
                    SharedCryptoError::UnlockedKeyNotFound {
                        key_id: locked.id.clone(),
                    },
                )?;
                let local = LocalUserKey::relock_user_key(self.pgp, decrypted, new_passphrase)?;
                Ok(LtCoreResetAuthDevicesUserKey {
                    id: LtAuthUserKeyId(locked.id.0.clone()),
                    private_key: Sensitive::new(local.private_key.0.clone()),
                })
            })
            .collect()
    }

    pub fn collect_decrypt_keys_for_activation_address<'b>(
        &self,
        address_keys: &'b [UnlockedAddressKey<P>],
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
        user_keys: &[UnlockedUserKey<P>],
        key_passphrase: &KeySecret,
    ) -> Result<Vec<UnlockedAddressKey<P>>, SharedCryptoError> {
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
            .map(|v| v.concat())
    }

    /// Crypto payload for `POST .../members/{id}/devices/reset`.
    pub fn build_devices_reset_crypto(
        &self,
        member_keys: &UserKeys,
        unlocked_user_keys: &[UnlockedUserKey<P>],
        device_secret: &DeviceSecret,
        new_passphrase: &KeySecret,
        member_org_passphrase: Option<&KeySecret>,
        auth_device_id: LtCoreAuthDeviceId,
    ) -> Result<LtCorePostMembersDevicesResetBody, SharedCryptoError> {
        let current_unlock =
            self.member_primary_unlock_passphrase(member_keys, member_org_passphrase)?;
        let rearmored_user_keys =
            self.rearmor_user_keys(member_keys.as_ref(), unlocked_user_keys, new_passphrase)?;
        let encrypted_secret = EncryptedSecret::from_key_secret(&current_unlock, &device_secret.0)?;

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
            member_keys,
            &unlocked_user_keys,
            &device_secret,
            &secure_hex_key_secret_32(),
            member_org_passphrase,
            pending.id.clone(),
        )
    }

    pub fn unlock_org_managed_user_keys(
        &self,
        member_keys: &UserKeys,
        member_org_passphrase: Option<&KeySecret>,
    ) -> Result<Vec<UnlockedUserKey<P>>, SharedCryptoError> {
        let mut unlocked = Vec::new();
        for locked in member_keys.as_ref() {
            let passphrase = self.unlock_passphrase_for_locked(locked, member_org_passphrase)?;
            let private_key = self.pgp.private_key_import(
                locked.private_key.0.as_bytes(),
                passphrase.as_ref(),
                DataEncoding::Armor,
            )?;
            let public_key = self.pgp.private_key_to_public_key(&private_key)?;
            unlocked.push(UnlockedUserKey::<P> {
                id: locked.id.clone(),
                private_key,
                public_key,
            });
        }
        if unlocked.is_empty() {
            return Err(SharedCryptoError::NoMemberUserKeysUnlocked);
        }
        Ok(unlocked)
    }
}

fn locked_activation_token(locked: &LockedKey) -> Option<LtCoreUnprivActivationToken> {
    locked
        .activation
        .as_ref()
        .map(|token| LtCoreUnprivActivationToken(Sensitive::new(token.clone())))
}
