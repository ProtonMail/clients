use lattice::Sensitive;
use lattice::auth::LtAuthAddressId;
use lattice::core::LtCoreAddressesListRes;
use lattice::core::get_core_addresses::LtCoreGetAddressesReq;
use lattice::core::get_members::LtCoreGetMembersReq;
use lattice::core::get_organizations_keys::LtCoreGetOrganizationsKeysReq;
use lattice::core::keys::LtCoreGetKeySaltsReq;
use lattice::core::post_members_unprivatize::{
    LtCorePostMembersUnprivatizeBody, LtCorePostMembersUnprivatizeReq,
    LtCorePostMembersUnprivatizeRes,
};
use lattice::core::put_organizations_keys_signature::{
    LtCorePutOrganizationsKeysSignatureBody, LtCorePutOrganizationsKeysSignatureReq,
    LtCorePutOrganizationsKeysSignatureRes,
};
use lattice::core::unpriv_types::{LtCoreUnprivInvitationData, LtCoreUnprivInvitationSignature};
use lattice::core::user::get_users::LtCoreGetUsersReq;
use proton_crypto::crypto::{
    AccessKeyInfo, DataEncoding, Encryptor, EncryptorSync, PGPProviderSync, Signer, SignerSync,
    UnixTimestamp,
};
use proton_crypto_account::keys::{UnlockedAddressKey, UnlockedUserKey};
use proton_crypto_account::salts::KeySecret;

use super::super::Session;
use super::super::org_members::{
    decrypt_org_armored_token, derive_key_passphrase as org_derive_key_passphrase,
    find_member_by_email, primary_key_id,
};
use super::member_keys_unpriv::MemberKeysUnprivReady;
use super::unprivatize_admin_error::UnprivatizeAdminError;

const ORG_FINGERPRINT_CONTEXT: &str = "account.organization-fingerprint";
const UNPRIVATIZATION_INVITE_CONTEXT: &str = "account.unprivatization-invitation-data";

/// Unlocked org key + admin user keys, from [`AdminPgpState::load`].
pub struct AdminPgpState<P: PGPProviderSync> {
    pub pgp: P,
    pub key_passphrase: KeySecret,
    pub user_keys: Vec<UnlockedUserKey<P>>,
    pub org_private: P::PrivateKey,
}

impl<P: PGPProviderSync> AdminPgpState<P> {
    pub async fn load(
        pgp: P,
        session: &Session,
        admin_password: &str,
    ) -> Result<Self, UnprivatizeAdminError> {
        let admin_user = session.send_lt(LtCoreGetUsersReq).await?.user;
        let primary_id = primary_key_id(&admin_user)?;
        let key_salts = session.send_lt(LtCoreGetKeySaltsReq).await?.key_salts;
        let key_passphrase =
            org_derive_key_passphrase(&key_salts, &primary_id, admin_password.as_bytes())
                .map_err(UnprivatizeAdminError::KeyPassphrase)?;
        let unlock = admin_user.keys.unlock(&pgp, &key_passphrase);
        if unlock.unlocked_keys.is_empty() {
            return Err(UnprivatizeAdminError::UserKeysNotUnlocked {
                failed: format!("{:?}", unlock.failed),
            });
        }
        let user_keys = unlock.unlocked_keys;
        let org_res = session.send_lt(LtCoreGetOrganizationsKeysReq).await?;
        let org_armor = org_res
            .private_key
            .as_ref()
            .ok_or(UnprivatizeAdminError::NoOrgPrivateKey)?;
        let org_private = pgp
            .private_key_import(
                org_armor.as_str().as_bytes(),
                key_passphrase.as_ref(),
                DataEncoding::Armor,
            )
            .map_err(|e| UnprivatizeAdminError::PgpImportOrDerive(e.to_string()))?;
        Ok(Self {
            pgp,
            key_passphrase,
            user_keys,
            org_private,
        })
    }

    pub async fn publish_org_identity(
        &self,
        session: &Session,
    ) -> Result<(), UnprivatizeAdminError> {
        let fp_hex = self.hex_org_fingerprint()?;
        let addrs = session.send_lt(LtCoreGetAddressesReq::default()).await?;
        let (address_id, primary_addr) = self.pick_primary_address_unlocked_key(&addrs)?;
        let armored = self.sign_fingerprint_utc0(&primary_addr.private_key, fp_hex.as_bytes())?;
        let _: LtCorePutOrganizationsKeysSignatureRes = session
            .send_lt(LtCorePutOrganizationsKeysSignatureReq {
                body: LtCorePutOrganizationsKeysSignatureBody {
                    signature: Sensitive::new(armored),
                    address_id,
                },
            })
            .await?;
        Ok(())
    }

    pub async fn unprivatize_member(
        &self,
        session: &Session,
        member_email: &str,
    ) -> Result<(), UnprivatizeAdminError> {
        let members = session.send_lt(LtCoreGetMembersReq::default()).await?;
        let member = find_member_by_email(&members, member_email)?;
        let invitation_data =
            format!(r#"{{"Address":"{member_email}", "Revision":1, "Admin":false}}"#);
        let inv_sig = self.sign_invitation(invitation_data.as_bytes())?;
        let _: LtCorePostMembersUnprivatizeRes = session
            .send_lt(LtCorePostMembersUnprivatizeReq {
                member_id: member.id.clone(),
                body: LtCorePostMembersUnprivatizeBody {
                    invitation_data: LtCoreUnprivInvitationData(invitation_data),
                    invitation_signature: LtCoreUnprivInvitationSignature(Sensitive::new(inv_sig)),
                },
            })
            .await?;
        Ok(())
    }

    pub async fn complete_member_keys_unprivatization(
        &self,
        session: &Session,
        member_email: &str,
    ) -> Result<KeySecret, UnprivatizeAdminError> {
        let ready = MemberKeysUnprivReady::fetch(session, member_email).await?;
        ready.submit(session, self).await
    }

    pub(crate) fn decrypt_setup_activation_token(
        &self,
        activation_armored: &str,
    ) -> Result<KeySecret, UnprivatizeAdminError> {
        decrypt_org_armored_token(&self.pgp, &self.org_private, activation_armored, false)
            .map_err(UnprivatizeAdminError::PgpImportOrDerive)
    }

    pub(crate) fn encrypt_org_only_token(
        &self,
        org_random_token: &KeySecret,
    ) -> Result<String, UnprivatizeAdminError> {
        let org_public = self
            .pgp
            .private_key_to_public_key(&self.org_private)
            .map_err(|e| UnprivatizeAdminError::PgpImportOrDerive(e.to_string()))?;
        let encrypted = self
            .pgp
            .new_encryptor()
            .with_encryption_key(&org_public)
            .encrypt_raw(org_random_token.as_ref(), DataEncoding::Armor)
            .map_err(|e| UnprivatizeAdminError::PgpImportOrDerive(e.to_string()))?;
        String::from_utf8(encrypted).map_err(UnprivatizeAdminError::PgpArmoredNotUtf8)
    }

    fn hex_org_fingerprint(&self) -> Result<String, UnprivatizeAdminError> {
        let org_public = self
            .pgp
            .private_key_to_public_key(&self.org_private)
            .map_err(|e| UnprivatizeAdminError::PgpImportOrDerive(e.to_string()))?;
        org_public
            .sha256_key_fingerprints()
            .first()
            .ok_or(UnprivatizeAdminError::NoOrgSha256Fingerprint)
            .map(|fp| fp.to_string())
    }

    fn pick_primary_address_unlocked_key(
        &self,
        addrs: &LtCoreAddressesListRes,
    ) -> Result<(LtAuthAddressId, UnlockedAddressKey<P>), UnprivatizeAdminError> {
        let primary_address = addrs
            .addresses
            .iter()
            .min_by_key(|a| a.order)
            .ok_or(UnprivatizeAdminError::NoAddresses)?;
        let addr_id = primary_address.id.clone();
        let addr_unlock =
            primary_address
                .keys
                .0
                .unlock(&self.pgp, &self.user_keys, Some(&self.key_passphrase));
        let primary = addr_unlock
            .unlocked_keys
            .into_iter()
            .find(|k| k.primary)
            .ok_or(UnprivatizeAdminError::NoPrimaryAddressKey)?;
        Ok((addr_id, primary))
    }

    fn sign_fingerprint_utc0(
        &self,
        address_private: &P::PrivateKey,
        data: &[u8],
    ) -> Result<String, UnprivatizeAdminError> {
        let ctx = self
            .pgp
            .new_signing_context(ORG_FINGERPRINT_CONTEXT.to_owned(), true);
        let bytes = self
            .pgp
            .new_signer()
            .with_signing_key(address_private)
            .with_signing_context(&ctx)
            .at_signing_time(UnixTimestamp::zero())
            .sign_detached(data, DataEncoding::Armor)
            .map_err(|e| UnprivatizeAdminError::PgpSignFingerprint(e.to_string()))?;
        String::from_utf8(bytes).map_err(UnprivatizeAdminError::PgpArmoredNotUtf8)
    }

    fn sign_invitation(&self, data: &[u8]) -> Result<String, UnprivatizeAdminError> {
        let ctx = self
            .pgp
            .new_signing_context(UNPRIVATIZATION_INVITE_CONTEXT.to_owned(), true);
        let bytes = self
            .pgp
            .new_signer()
            .with_signing_key(&self.org_private)
            .with_signing_context(&ctx)
            .sign_detached(data, DataEncoding::Armor)
            .map_err(|e| UnprivatizeAdminError::PgpSignInvitation(e.to_string()))?;
        String::from_utf8(bytes).map_err(UnprivatizeAdminError::PgpArmoredNotUtf8)
    }
}
