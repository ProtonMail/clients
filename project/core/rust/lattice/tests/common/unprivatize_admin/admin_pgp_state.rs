use core_key::{LockedKeysExt, OrgAdminPgp};
use lattice::Sensitive;
use lattice::auth::LtAuthAddressId;
use lattice::core::LtCoreAddress;
use lattice::core::get_core_addresses::LtCoreGetAddressesReq;
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
    AccessKeyInfo, DataEncoding, PGPProviderSync, Signer, SignerSync, UnixTimestamp,
};
use proton_crypto::new_srp_provider;
use proton_crypto_account::keys::{UnlockedAddressKey, UnlockedUserKey};
use proton_crypto_account::salts::KeySecret;

use super::super::Session;
use super::member_keys_unpriv::complete_member_keys_unprivatization;
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
        let primary_id = admin_user
            .keys
            .primary_key_id()
            .cloned()
            .ok_or(UnprivatizeAdminError::NoPrimaryUserKey)?;
        let key_salts = session.send_lt(LtCoreGetKeySaltsReq).await?.key_salts;
        let key_passphrase = key_salts
            .salt_for_key(&new_srp_provider(), &primary_id, admin_password.as_bytes())
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
        let (address_id, primary_addr) =
            self.pick_primary_address_unlocked_key(&addrs.addresses)?;
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
        let member = session.find_member_by_email(member_email).await?;
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
        complete_member_keys_unprivatization(session, member_email, self).await
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
        addrs: &[LtCoreAddress],
    ) -> Result<(LtAuthAddressId, UnlockedAddressKey<P>), UnprivatizeAdminError> {
        let primary_address = addrs
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

    pub(crate) fn org_admin_pgp(&self) -> OrgAdminPgp<'_, P> {
        OrgAdminPgp::new(&self.pgp, &self.org_private, &self.key_passphrase)
    }
}
