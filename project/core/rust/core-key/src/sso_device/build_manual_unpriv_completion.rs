//! Manual admin unprivatization (Appendix B0) shared by account-crux and lattice tests.

use lattice::Sensitive;
use lattice::core::get_members::LtCoreMemberListUnprivatization;
use lattice::core::post_members_keys_unprivatize::{
    LtCorePostMembersKeysUnprivatizeBody, LtCoreUnprivatizeAddressKey, LtCoreUnprivatizeUserKey,
};
use lattice::core::{LtCoreAddress, LtCoreUnprivActivationToken, LtCoreUnprivState};
use proton_crypto::crypto::{
    AsPublicKeyRef, DataEncoding, Decryptor, DecryptorSync, DetachedSignatureVariant, Encryptor,
    EncryptorSync, PGPProviderSync, Signer, SignerSync, SigningMode, VerifiedData, WritingMode,
};
use proton_crypto_account::keys::{EncryptedKeyToken, KeyId, KeyTokenSignature, UnlockedUserKey};
use proton_crypto_account::salts::KeySecret;

use crate::sso_device::secure_hex_key_secret_32;
use crate::{OrgAdminPgp, SharedCryptoError};

const ADDRESS_ORG_SIGNATURE_CONTEXT: &str = "account.key-token.address";

impl<'a, P: PGPProviderSync> OrgAdminPgp<'a, P> {
    pub fn build_manual_unpriv_completion(
        &self,
        member_label: &str,
        unpriv: &LtCoreMemberListUnprivatization,
        addrs: &[LtCoreAddress],
    ) -> Result<(LtCorePostMembersKeysUnprivatizeBody, KeySecret), SharedCryptoError> {
        let private_keys =
            unpriv
                .private_keys()
                .ok_or_else(|| SharedCryptoError::MissingPrivateKeys {
                    member_label: member_label.to_string(),
                })?;
        let activation_token = unpriv.activation_token.as_ref().ok_or_else(|| {
            SharedCryptoError::MissingActivationToken {
                member_label: member_label.to_string(),
            }
        })?;
        let org_random_token = secure_hex_key_secret_32();
        let org_token_armored = self.encrypt_org_armored_token(&org_random_token)?;
        let body = self.build_manual_unprivatize_body(
            &private_keys,
            activation_token,
            addrs,
            &org_random_token,
            &org_token_armored,
        )?;
        Ok((body, org_random_token))
    }

    pub(crate) fn build_manual_unprivatize_body(
        &self,
        private_keys: &[Sensitive<String>],
        activation_token_armored: &LtCoreUnprivActivationToken,
        addrs: &[LtCoreAddress],
        org_random_token: &KeySecret,
        org_token_armored: &str,
    ) -> Result<LtCorePostMembersKeysUnprivatizeBody, SharedCryptoError> {
        let random_token = self.decrypt_org_armored_token(activation_token_armored, false)?;
        let member_user_keys = self.unlock_armored_private_keys(private_keys, &random_token)?;
        let user_keys = self.build_unpriv_user_keys(
            private_keys,
            &random_token,
            org_random_token,
            org_token_armored,
        )?;
        let address_keys = self.build_unpriv_address_keys(addrs, &member_user_keys)?;
        Ok(LtCorePostMembersKeysUnprivatizeBody {
            user_keys,
            address_keys,
            organization_key_activation: None,
        })
    }

    fn build_unpriv_address_keys(
        &self,
        addrs: &[LtCoreAddress],
        member_user_keys: &[UnlockedUserKey<P>],
    ) -> Result<Vec<LtCoreUnprivatizeAddressKey>, SharedCryptoError> {
        use data_encoding::BASE64;

        let org_public = self.pgp.private_key_to_public_key(self.org_private)?;
        let decryption_keys: Vec<_> = member_user_keys.iter().map(|k| &k.private_key).collect();
        let verification_keys: Vec<_> =
            member_user_keys.iter().map(|k| k.as_public_key()).collect();
        let sign_ctx = self
            .pgp
            .new_signing_context(ADDRESS_ORG_SIGNATURE_CONTEXT.to_owned(), true);

        addrs
            .iter()
            .flat_map(|address| address.keys.0.as_ref().iter())
            .map(|locked| {
                let token =
                    locked
                        .token
                        .as_ref()
                        .ok_or_else(|| SharedCryptoError::MissingToken {
                            id: locked.id.0.clone(),
                        })?;
                let signature = locked.signature.as_ref().ok_or_else(|| {
                    SharedCryptoError::MissingSignature {
                        id: locked.id.0.clone(),
                    }
                })?;

                let secret = self.decrypt_address_key_token(
                    token,
                    signature,
                    &decryption_keys,
                    &verification_keys,
                )?;

                let mut encrypted_body = Vec::new();
                let detached = self
                    .pgp
                    .new_encryptor()
                    .with_encryption_key(&org_public)
                    .encrypt_to_writer(
                        std::io::Cursor::new(secret.as_slice()),
                        DataEncoding::Bytes,
                        SigningMode::Inline,
                        WritingMode::SplitKeyPackets,
                        &mut encrypted_body,
                    )?;
                let key_packets = detached.try_as_key_packets()?;
                let org_token_key_packet = BASE64.encode(key_packets);

                let org_signature_bytes = self
                    .pgp
                    .new_signer()
                    .with_signing_key(self.org_private)
                    .with_signing_context(&sign_ctx)
                    .sign_detached(secret.as_slice(), DataEncoding::Armor)?;
                let org_signature = String::from_utf8(org_signature_bytes)?;

                Ok(LtCoreUnprivatizeAddressKey {
                    address_key_id: locked.id.0.clone(),
                    org_token_key_packet: Sensitive::new(org_token_key_packet),
                    org_signature: Sensitive::new(org_signature),
                })
            })
            .collect()
    }

    fn unlock_armored_private_keys(
        &self,
        private_keys: &[Sensitive<String>],
        random_token: &KeySecret,
    ) -> Result<Vec<UnlockedUserKey<P>>, SharedCryptoError> {
        private_keys
            .iter()
            .map(|armored| {
                let private_key = self.import_armored_private_key(armored, random_token)?;
                let public_key = self.pgp.private_key_to_public_key(&private_key)?;
                Ok(UnlockedUserKey::<P> {
                    id: KeyId(String::new()),
                    private_key,
                    public_key,
                })
            })
            .collect()
    }

    fn build_unpriv_user_keys(
        &self,
        private_keys: &[Sensitive<String>],
        random_token: &KeySecret,
        org_random_token: &KeySecret,
        org_token_armored: &str,
    ) -> Result<Vec<LtCoreUnprivatizeUserKey>, SharedCryptoError> {
        private_keys
            .iter()
            .map(|armored| {
                let private_key = self.import_armored_private_key(armored, random_token)?;
                let exported = self.pgp.private_key_export(
                    &private_key,
                    org_random_token.as_ref(),
                    DataEncoding::Armor,
                )?;
                let org_private_key = String::from_utf8(exported.as_ref().to_vec())?;
                Ok(LtCoreUnprivatizeUserKey {
                    org_private_key: Sensitive::new(org_private_key),
                    org_token: Sensitive::new(org_token_armored.to_owned()),
                })
            })
            .collect()
    }

    fn decrypt_address_key_token(
        &self,
        token: &EncryptedKeyToken,
        signature: &KeyTokenSignature,
        decryption_keys: &[&P::PrivateKey],
        verification_keys: &[&P::PublicKey],
    ) -> Result<Vec<u8>, SharedCryptoError> {
        let verified = self
            .pgp
            .new_decryptor()
            .with_decryption_key_refs(decryption_keys)
            .with_verification_key_refs(verification_keys)
            .with_detached_signature_ref(
                signature.0.as_bytes(),
                DetachedSignatureVariant::Plaintext,
                true,
            )
            .decrypt(token.0.as_bytes(), DataEncoding::Armor)?;
        verified.verification_result()?;
        Ok(verified.to_vec())
    }

    fn import_armored_private_key(
        &self,
        armored: &Sensitive<String>,
        passphrase: &KeySecret,
    ) -> Result<P::PrivateKey, SharedCryptoError> {
        self.pgp
            .private_key_import(
                armored.as_str().as_bytes(),
                passphrase.as_ref(),
                DataEncoding::Armor,
            )
            .map_err(SharedCryptoError::Crypto)
    }
}

pub trait LtCoreMemberListUnprivatizationExt {
    fn private_keys(&self) -> Option<Vec<Sensitive<String>>>;
    fn ready_for_admin_keys_completion(&self) -> bool;
}

impl LtCoreMemberListUnprivatizationExt for LtCoreMemberListUnprivatization {
    fn private_keys(&self) -> Option<Vec<Sensitive<String>>> {
        match &self.private_keys {
            Some(keys) if !keys.is_empty() => Some(keys.clone()),
            _ => self.private_key.as_ref().map(|pk| vec![pk.0.clone()]),
        }
    }
    /// Ready for admin `POST .../keys/unprivatize` (invitation fields may remain).
    fn ready_for_admin_keys_completion(&self) -> bool {
        self.state == Some(LtCoreUnprivState::Ready)
            && self
                .activation_token
                .as_ref()
                .is_some_and(|t| !t.0.is_empty())
            && self
                .private_keys
                .as_ref()
                .is_some_and(|keys| !keys.is_empty())
    }
}
