use lattice::Sensitive;
use lattice::core::LtCoreAddressesListRes;
use lattice::core::LtCoreMemberEncId;
use lattice::core::get_members::LtCoreGetMembersReq;
use lattice::core::post_members_keys_unprivatize::{
    LtCorePostMembersKeysUnprivatizeBody, LtCorePostMembersKeysUnprivatizeReq,
    LtCoreUnprivatizeAddressKey, LtCoreUnprivatizeUserKey,
};
use lattice::core::unpriv_types::LtCoreUnprivState;
use proton_crypto::crypto::{
    AsPublicKeyRef, DataEncoding, Decryptor, DecryptorSync, DetachedSignatureVariant, Encryptor,
    EncryptorSync, PGPProviderSync, Signer, SignerSync, SigningMode, VerifiedData, WritingMode,
};
use proton_crypto_account::keys::{EncryptedKeyToken, KeyId, KeyTokenSignature, UnlockedUserKey};
use proton_crypto_account::salts::KeySecret;

use super::super::Session;
use super::super::org_members::{
    fetch_member_addresses_paginated, find_member_by_email, organization_token_from_random,
};
use super::admin_pgp_state::AdminPgpState;
use super::unprivatize_admin_error::UnprivatizeAdminError;

const ADDRESS_ORG_SIGNATURE_CONTEXT: &str = "account.key-token.address";

pub(crate) struct MemberKeysUnprivReady {
    member_id: LtCoreMemberEncId,
    private_keys: Vec<Sensitive<String>>,
    activation_token: Sensitive<String>,
    addrs: LtCoreAddressesListRes,
}

impl MemberKeysUnprivReady {
    pub async fn fetch(
        session: &Session,
        member_email: &str,
    ) -> Result<Self, UnprivatizeAdminError> {
        let members = session.send_lt(LtCoreGetMembersReq::default()).await?;
        let member = find_member_by_email(&members, member_email)?;
        let unpriv = match member.unprivatization.as_ref() {
            Some(u) if u.state == Some(LtCoreUnprivState::Ready) => u,
            _ => {
                return Err(UnprivatizeAdminError::UnprivatizationNotReady {
                    email: member_email.to_string(),
                });
            }
        };

        let private_keys = Self::armored_private_keys_from_unpriv(member_email, unpriv)?;
        let activation_token = unpriv
            .activation_token
            .as_ref()
            .ok_or(UnprivatizeAdminError::MissingUnprivActivationToken {
                email: member_email.to_string(),
            })?
            .0
            .clone();

        let addrs = fetch_member_addresses_paginated(session, &member.id).await?;

        Ok(Self {
            member_id: member.id.clone(),
            private_keys,
            activation_token,
            addrs,
        })
    }

    pub async fn submit<P: PGPProviderSync>(
        self,
        session: &Session,
        admin: &AdminPgpState<P>,
    ) -> Result<KeySecret, UnprivatizeAdminError> {
        let pgp = &admin.pgp;
        let random_token = admin.decrypt_setup_activation_token(self.activation_token.as_str())?;

        let member_user_keys =
            Self::unlock_member_user_keys(pgp, &self.private_keys, &random_token)?;

        let org_random_token = organization_token_from_random();
        let org_token_armored = admin.encrypt_org_only_token(&org_random_token)?;

        let user_keys = Self::build_unpriv_user_keys(
            pgp,
            &self.private_keys,
            &random_token,
            &org_random_token,
            &org_token_armored,
        )?;

        let address_keys = Self::build_unpriv_address_keys(
            pgp,
            &admin.org_private,
            &self.addrs,
            &member_user_keys,
        )?;

        let body = LtCorePostMembersKeysUnprivatizeBody {
            user_keys,
            address_keys,
            organization_key_activation: None,
        };

        session
            .send_lt(LtCorePostMembersKeysUnprivatizeReq {
                member_id: self.member_id,
                body,
            })
            .await?;
        Ok(org_random_token)
    }

    fn armored_private_keys_from_unpriv(
        member_email: &str,
        unpriv: &lattice::core::get_members::LtCoreMemberListUnprivatization,
    ) -> Result<Vec<Sensitive<String>>, UnprivatizeAdminError> {
        match &unpriv.private_keys {
            Some(keys) if !keys.is_empty() => Ok(keys.clone()),
            _ => unpriv
                .private_key
                .as_ref()
                .map(|pk| vec![pk.0.clone()])
                .ok_or(UnprivatizeAdminError::MissingUnprivPrivateKeys {
                    email: member_email.to_string(),
                }),
        }
    }

    fn unlock_member_user_keys<P: PGPProviderSync>(
        pgp: &P,
        private_keys: &[Sensitive<String>],
        random_token: &KeySecret,
    ) -> Result<Vec<UnlockedUserKey<P>>, UnprivatizeAdminError> {
        private_keys
            .iter()
            .map(|armored| {
                let private_key = Self::import_armored_private_key(pgp, armored, random_token)?;
                let public_key = pgp
                    .private_key_to_public_key(&private_key)
                    .map_err(|e| UnprivatizeAdminError::PgpImportOrDerive(e.to_string()))?;
                Ok(UnlockedUserKey::<P> {
                    id: KeyId(String::new()),
                    private_key,
                    public_key,
                })
            })
            .collect()
    }

    fn build_unpriv_user_keys<P: PGPProviderSync>(
        pgp: &P,
        private_keys: &[Sensitive<String>],
        random_token: &KeySecret,
        org_random_token: &KeySecret,
        org_token_armored: &str,
    ) -> Result<Vec<LtCoreUnprivatizeUserKey>, UnprivatizeAdminError> {
        private_keys
            .iter()
            .map(|armored| {
                let private_key = Self::import_armored_private_key(pgp, armored, random_token)?;
                let exported = pgp
                    .private_key_export(
                        &private_key,
                        org_random_token.as_ref(),
                        DataEncoding::Armor,
                    )
                    .map_err(|e| UnprivatizeAdminError::PgpImportOrDerive(e.to_string()))?;
                let org_private_key = String::from_utf8(exported.as_ref().to_vec())
                    .map_err(UnprivatizeAdminError::PgpArmoredNotUtf8)?;
                Ok(LtCoreUnprivatizeUserKey {
                    org_private_key: Sensitive::new(org_private_key),
                    org_token: Sensitive::new(org_token_armored.to_owned()),
                })
            })
            .collect()
    }

    fn decrypt_address_key_token<P: PGPProviderSync>(
        pgp: &P,
        token: &EncryptedKeyToken,
        signature: &KeyTokenSignature,
        decryption_keys: &[&P::PrivateKey],
        verification_keys: &[&P::PublicKey],
    ) -> Result<Vec<u8>, UnprivatizeAdminError> {
        let verified = pgp
            .new_decryptor()
            .with_decryption_key_refs(decryption_keys)
            .with_verification_key_refs(verification_keys)
            .with_detached_signature_ref(
                signature.0.as_bytes(),
                DetachedSignatureVariant::Plaintext,
                true,
            )
            .decrypt(token.0.as_bytes(), DataEncoding::Armor)
            .map_err(|e| UnprivatizeAdminError::PgpImportOrDerive(e.to_string()))?;
        verified
            .verification_result()
            .map_err(|e| UnprivatizeAdminError::PgpImportOrDerive(e.to_string()))?;
        Ok(verified.to_vec())
    }

    fn build_unpriv_address_keys<P: PGPProviderSync>(
        pgp: &P,
        org_private: &P::PrivateKey,
        addrs: &LtCoreAddressesListRes,
        member_user_keys: &[UnlockedUserKey<P>],
    ) -> Result<Vec<LtCoreUnprivatizeAddressKey>, UnprivatizeAdminError> {
        use data_encoding::BASE64;

        let org_public = pgp
            .private_key_to_public_key(org_private)
            .map_err(|e| UnprivatizeAdminError::PgpImportOrDerive(e.to_string()))?;
        let decryption_keys: Vec<_> = member_user_keys.iter().map(|k| &k.private_key).collect();
        let verification_keys: Vec<_> =
            member_user_keys.iter().map(|k| k.as_public_key()).collect();
        let sign_ctx = pgp.new_signing_context(ADDRESS_ORG_SIGNATURE_CONTEXT.to_owned(), true);

        let mut address_keys = Vec::new();
        for address in &addrs.addresses {
            for locked in address.keys.0.as_ref() {
                let token = locked.token.as_ref().ok_or_else(|| {
                    UnprivatizeAdminError::PgpImportOrDerive(format!(
                        "address key {} missing token",
                        locked.id.0
                    ))
                })?;
                let signature = locked.signature.as_ref().ok_or_else(|| {
                    UnprivatizeAdminError::PgpImportOrDerive(format!(
                        "address key {} missing signature",
                        locked.id.0
                    ))
                })?;

                let secret = Self::decrypt_address_key_token(
                    pgp,
                    token,
                    signature,
                    &decryption_keys,
                    &verification_keys,
                )?;

                let mut encrypted_body = Vec::new();
                let detached = pgp
                    .new_encryptor()
                    .with_encryption_key(&org_public)
                    .encrypt_to_writer(
                        std::io::Cursor::new(secret.as_slice()),
                        DataEncoding::Bytes,
                        SigningMode::Inline,
                        WritingMode::SplitKeyPackets,
                        &mut encrypted_body,
                    )
                    .map_err(|e| UnprivatizeAdminError::PgpImportOrDerive(e.to_string()))?;
                let key_packets = detached
                    .try_as_key_packets()
                    .map_err(|e| UnprivatizeAdminError::PgpImportOrDerive(e.to_string()))?;
                let org_token_key_packet = BASE64.encode(key_packets);

                let org_signature_bytes = pgp
                    .new_signer()
                    .with_signing_key(org_private)
                    .with_signing_context(&sign_ctx)
                    .sign_detached(secret.as_slice(), DataEncoding::Armor)
                    .map_err(|e| UnprivatizeAdminError::PgpImportOrDerive(e.to_string()))?;
                let org_signature = String::from_utf8(org_signature_bytes)
                    .map_err(UnprivatizeAdminError::PgpArmoredNotUtf8)?;

                address_keys.push(LtCoreUnprivatizeAddressKey {
                    address_key_id: locked.id.0.clone(),
                    org_token_key_packet: Sensitive::new(org_token_key_packet),
                    org_signature: Sensitive::new(org_signature),
                });
            }
        }
        Ok(address_keys)
    }

    fn import_armored_private_key<P: PGPProviderSync>(
        pgp: &P,
        armored: &Sensitive<String>,
        passphrase: &KeySecret,
    ) -> Result<P::PrivateKey, UnprivatizeAdminError> {
        pgp.private_key_import(
            armored.as_str().as_bytes(),
            passphrase.as_ref(),
            DataEncoding::Armor,
        )
        .map_err(|e| UnprivatizeAdminError::PgpImportOrDerive(e.to_string()))
    }
}
