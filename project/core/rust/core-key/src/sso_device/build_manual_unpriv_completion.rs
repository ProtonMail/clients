//! Manual admin unprivatization (Appendix B0) shared by account-crux and lattice tests.

use lattice::Sensitive;
use lattice::core::get_members::LtCoreMemberListUnprivatization;
use lattice::core::post_members_keys_unprivatize::{
    LtCorePostMembersKeysUnprivatizeBody, LtCoreUnprivatizeAddressKey, LtCoreUnprivatizeUserKey,
};
use lattice::core::{LtCoreAddress, LtCoreUnprivActivationToken, LtCoreUnprivState};
use proton_crypto::crypto::{
    DataEncoding, Encryptor, EncryptorSync, PGPProviderSync, Signer, SignerSync, SigningMode,
    WritingMode,
};
use proton_crypto_account::keys::{ArmoredPrivateKey, KeyId, UnlockedUserKeys};
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
        private_keys: &[ArmoredPrivateKey],
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
        member_user_keys: &UnlockedUserKeys<P>,
    ) -> Result<Vec<LtCoreUnprivatizeAddressKey>, SharedCryptoError> {
        use data_encoding::BASE64;

        let org_public = self.public_key()?;

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

                let secret =
                    self.decrypt_signed_armored_token(token, signature, member_user_keys)?;

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
        private_keys: &[ArmoredPrivateKey],
        random_token: &KeySecret,
    ) -> Result<UnlockedUserKeys<P>, SharedCryptoError> {
        private_keys
            .iter()
            .map(|armored| {
                self.unlock_user_key_from_armored(armored, random_token, KeyId(String::new()))
            })
            .collect::<Result<Vec<_>, _>>()
            .map(UnlockedUserKeys::from)
    }

    fn build_unpriv_user_keys(
        &self,
        private_keys: &[ArmoredPrivateKey],
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
}

pub trait LtCoreMemberListUnprivatizationExt {
    fn private_keys(&self) -> Option<Vec<ArmoredPrivateKey>>;
    fn is_ready_for_manual_admin_completion(&self) -> bool;
}

impl LtCoreMemberListUnprivatizationExt for LtCoreMemberListUnprivatization {
    fn private_keys(&self) -> Option<Vec<ArmoredPrivateKey>> {
        match &self.private_keys {
            Some(keys) if !keys.is_empty() => Some(
                keys.iter()
                    .map(|k| ArmoredPrivateKey(k.0.clone().into_inner()))
                    .collect(),
            ),
            _ => self
                .private_key
                .as_ref()
                .map(|pk| vec![ArmoredPrivateKey(pk.0.clone().into_inner())]),
        }
    }
    /// Ready for admin manual `POST .../keys/unprivatize`.
    ///
    /// Manual-approve only. The automatic-approve shape (non-empty `InvitationData` /
    /// `InvitationSignature`, or `PrivateIntent == true`) is rejected here so it is not
    /// processed with the wrong (manual) crypto. This codebase has no automatic-approve
    /// completion path, so a rejected member is not rerouted to an alternative; it just
    /// skips manual completion. Whether automatic-approve members should reach the admin
    /// device-approval flow at all is a separate question for the flow owner.
    ///
    /// `PrivateIntent` and the invitation fields use truthy/falsy checks (absent or empty
    /// is admitted), mirroring the reference web client gate.
    fn is_ready_for_manual_admin_completion(&self) -> bool {
        self.state == Some(LtCoreUnprivState::Ready)
            && self.private_intent != Some(true)
            && self.invitation_data.as_ref().is_none_or(|d| d.0.is_empty())
            && self
                .invitation_signature
                .as_ref()
                .is_none_or(|s| s.0.is_empty())
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

#[cfg(test)]
mod tests {
    use super::*;
    use lattice::core::get_members::LtCoreMemberListUnprivatization;
    use lattice::core::{
        LtCoreUnprivArmoredPrivateKey, LtCoreUnprivInvitationData, LtCoreUnprivInvitationSignature,
    };

    fn manual_ready() -> LtCoreMemberListUnprivatization {
        LtCoreMemberListUnprivatization {
            state: Some(LtCoreUnprivState::Ready),
            invitation_data: None,
            invitation_signature: None,
            invitation_email: None,
            private_key: None,
            private_keys: Some(vec![LtCoreUnprivArmoredPrivateKey("key".into())]),
            activation_token: Some(LtCoreUnprivActivationToken("token".into())),
            private_intent: Some(false),
        }
    }

    #[test]
    fn manual_ready_baseline_passes() {
        assert!(manual_ready().is_ready_for_manual_admin_completion());
    }

    #[test]
    fn absent_private_intent_passes() {
        let mut u = manual_ready();
        u.private_intent = None;
        assert!(u.is_ready_for_manual_admin_completion());
    }

    #[test]
    fn private_intent_true_is_rejected() {
        let mut u = manual_ready();
        u.private_intent = Some(true);
        assert!(!u.is_ready_for_manual_admin_completion());
    }

    #[test]
    fn non_empty_invitation_data_is_rejected() {
        let mut u = manual_ready();
        u.invitation_data = Some(LtCoreUnprivInvitationData("data".to_string()));
        assert!(!u.is_ready_for_manual_admin_completion());
    }

    #[test]
    fn non_empty_invitation_signature_is_rejected() {
        let mut u = manual_ready();
        u.invitation_signature = Some(LtCoreUnprivInvitationSignature("sig".into()));
        assert!(!u.is_ready_for_manual_admin_completion());
    }

    #[test]
    fn empty_invitation_fields_pass() {
        let mut u = manual_ready();
        u.invitation_data = Some(LtCoreUnprivInvitationData(String::new()));
        u.invitation_signature = Some(LtCoreUnprivInvitationSignature("".into()));
        assert!(u.is_ready_for_manual_admin_completion());
    }

    #[test]
    fn non_ready_state_is_rejected() {
        let mut u = manual_ready();
        u.state = Some(LtCoreUnprivState::Pending);
        assert!(!u.is_ready_for_manual_admin_completion());

        u.state = None;
        assert!(!u.is_ready_for_manual_admin_completion());
    }

    #[test]
    fn missing_or_empty_activation_token_is_rejected() {
        let mut u = manual_ready();
        u.activation_token = None;
        assert!(!u.is_ready_for_manual_admin_completion());

        u.activation_token = Some(LtCoreUnprivActivationToken("".into()));
        assert!(!u.is_ready_for_manual_admin_completion());
    }

    #[test]
    fn missing_or_empty_private_keys_is_rejected() {
        let mut u = manual_ready();
        u.private_keys = None;
        assert!(!u.is_ready_for_manual_admin_completion());

        u.private_keys = Some(vec![]);
        assert!(!u.is_ready_for_manual_admin_completion());
    }
}
