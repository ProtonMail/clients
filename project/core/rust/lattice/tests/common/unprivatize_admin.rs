//! Admin-side unprivatization: org-identity signature + `POST /members/{id}/unprivatize`.
//! Spacing in `invitation_data` must match server-side verification (see Account `GetMemberUnprivatizationOutput`).

use std::fmt;
use std::string::FromUtf8Error;

use lattice::auth::LtAuthAddressId;
use lattice::core::get_core_addresses::{LtCoreGetAddressesReq, LtCoreGetAddressesRes};
use lattice::core::get_members::{LtCoreGetMembersReq, LtCoreGetMembersRes, LtCoreMemberInfo};
use lattice::core::get_organizations_keys::{
    LtCoreGetOrganizationsKeysReq, LtCoreGetOrganizationsKeysRes,
};
use lattice::core::keys::{LtCoreGetKeySaltsReq, LtCoreGetKeysSaltsRes};
use lattice::core::post_members_unprivatize::{
    LtCorePostMembersUnprivatizeBody, LtCorePostMembersUnprivatizeReq,
};
use lattice::core::put_organizations_keys_signature::{
    LtCorePutOrganizationsKeysSignatureBody, LtCorePutOrganizationsKeysSignatureReq,
};
use lattice::core::unpriv_types::{LtCoreUnprivInvitationData, LtCoreUnprivInvitationSignature};
use lattice::core::user::LtCoreUser;
use lattice::core::user::get_users::{LtCoreGetUsersReq, LtCoreGetUsersRes};
use lattice::{LatticeError, Sensitive};
use proton_crypto::crypto::{
    AccessKeyInfo, DataEncoding, PGPProvider, PGPProviderSync, Signer, SignerSync, UnixTimestamp,
};
use proton_crypto::new_pgp_provider;
use proton_crypto::new_srp_provider;
use proton_crypto_account::keys::KeyId;
use proton_crypto_account::keys::UnlockedAddressKey;
use proton_crypto_account::keys::UnlockedUserKey;
use proton_crypto_account::salts::{KeySecret, SaltError};

use super::Session;

const ORG_FINGERPRINT_CONTEXT: &str = "account.organization-fingerprint";
const UNPRIVATIZATION_INVITE_CONTEXT: &str = "account.unprivatization-invitation-data";

/// High-level errors for the admin PGP + core flow used in unprivatization tests.
#[derive(Debug)]
pub enum UnprivatizeAdminError {
    Core(LatticeError),
    KeyPassphrase(SaltError),
    NoPrimaryUserKey,
    UserKeysNotUnlocked { failed: String },
    NoOrgPrivateKey,
    PgpImportOrDerive(String),
    NoOrgSha256Fingerprint,
    PgpSignFingerprint(String),
    PgpSignInvitation(String),
    NoAddresses,
    NoPrimaryAddressKey,
    PgpArmoredNotUtf8(FromUtf8Error),
    MemberNotFound { email: String, num_members: usize },
}

impl fmt::Display for UnprivatizeAdminError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UnprivatizeAdminError::Core(e) => write!(f, "{e}"),
            UnprivatizeAdminError::KeyPassphrase(e) => write!(f, "key passphrase: {e}"),
            UnprivatizeAdminError::NoPrimaryUserKey => f.write_str("user has no primary PGP key"),
            UnprivatizeAdminError::UserKeysNotUnlocked { failed } => {
                write!(f, "user keys not unlocked: {failed}")
            }
            UnprivatizeAdminError::NoOrgPrivateKey => {
                f.write_str("GET /organizations/keys: PrivateKey is absent")
            }
            UnprivatizeAdminError::PgpImportOrDerive(m) => write!(f, "PGP import/derive: {m}"),
            UnprivatizeAdminError::NoOrgSha256Fingerprint => {
                f.write_str("org public key: no SHA-256 fingerprint")
            }
            UnprivatizeAdminError::PgpSignFingerprint(m) => write!(f, "sign org fingerprint: {m}"),
            UnprivatizeAdminError::PgpSignInvitation(m) => write!(f, "sign invitation: {m}"),
            UnprivatizeAdminError::NoAddresses => f.write_str("user has no addresses"),
            UnprivatizeAdminError::NoPrimaryAddressKey => f.write_str("no primary address key"),
            UnprivatizeAdminError::PgpArmoredNotUtf8(e) => {
                write!(f, "armored PGP is not UTF-8: {e}")
            }
            UnprivatizeAdminError::MemberNotFound { email, num_members } => write!(
                f,
                "member {email:?} not in org (member count: {num_members})"
            ),
        }
    }
}

impl std::error::Error for UnprivatizeAdminError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            UnprivatizeAdminError::KeyPassphrase(e) => Some(e),
            UnprivatizeAdminError::PgpArmoredNotUtf8(e) => Some(e),
            _ => None,
        }
    }
}

impl From<LatticeError> for UnprivatizeAdminError {
    fn from(e: LatticeError) -> Self {
        Self::Core(e)
    }
}

/// Unlocked org key + admin user keys, from [`load_admin_pgp_state`].
pub struct AdminPgpState<P: PGPProviderSync> {
    pub pgp: P,
    pub key_passphrase: KeySecret,
    pub user_keys: Vec<UnlockedUserKey<P>>,
    pub org_private: P::PrivateKey,
}

// --- sub-steps: core (HTTP) ---------------------------------------------------

async fn core_get_users(session: &Session) -> Result<LtCoreGetUsersRes, UnprivatizeAdminError> {
    session.send_lt(LtCoreGetUsersReq).await.map_err(Into::into)
}

async fn core_get_key_salts(
    session: &Session,
) -> Result<LtCoreGetKeysSaltsRes, UnprivatizeAdminError> {
    session
        .send_lt(LtCoreGetKeySaltsReq)
        .await
        .map_err(Into::into)
}

async fn core_get_org_keys(
    session: &Session,
) -> Result<LtCoreGetOrganizationsKeysRes, UnprivatizeAdminError> {
    session
        .send_lt(LtCoreGetOrganizationsKeysReq)
        .await
        .map_err(Into::into)
}

async fn core_get_addresses(
    session: &Session,
) -> Result<LtCoreGetAddressesRes, UnprivatizeAdminError> {
    session
        .send_lt(LtCoreGetAddressesReq)
        .await
        .map_err(Into::into)
}

async fn core_get_members(session: &Session) -> Result<LtCoreGetMembersRes, UnprivatizeAdminError> {
    session
        .send_lt(LtCoreGetMembersReq)
        .await
        .map_err(Into::into)
}

// --- sub-steps: key / PGP ------------------------------------------------------

fn find_primary_key_id(user: &LtCoreUser) -> Result<KeyId, UnprivatizeAdminError> {
    user.keys
        .0
        .0
        .iter()
        .find(|k| k.primary)
        .map(|k| k.id.clone())
        .ok_or(UnprivatizeAdminError::NoPrimaryUserKey)
}

fn derive_key_passphrase(
    key_salts: &proton_crypto_account::salts::Salts,
    primary_key_id: &KeyId,
    admin_password: &str,
) -> Result<KeySecret, UnprivatizeAdminError> {
    key_salts
        .salt_for_key(
            &new_srp_provider(),
            primary_key_id,
            admin_password.as_bytes(),
        )
        .map_err(UnprivatizeAdminError::KeyPassphrase)
}

fn unlock_user_user_keys<P: PGPProviderSync>(
    pgp: &P,
    user: &LtCoreUser,
    key_passphrase: &KeySecret,
) -> Result<Vec<UnlockedUserKey<P>>, UnprivatizeAdminError> {
    let unlock = user.keys.unlock(pgp, key_passphrase);
    if unlock.unlocked_keys.is_empty() {
        return Err(UnprivatizeAdminError::UserKeysNotUnlocked {
            failed: format!("{:?}", unlock.failed),
        });
    }
    Ok(unlock.unlocked_keys)
}

fn pgp_import_org_private_key<P: PGPProviderSync>(
    pgp: &P,
    org_pk_armor: &str,
    key_passphrase: &KeySecret,
) -> Result<P::PrivateKey, UnprivatizeAdminError> {
    pgp.private_key_import(
        org_pk_armor.as_bytes(),
        key_passphrase.as_ref(),
        DataEncoding::Armor,
    )
    .map_err(|e| UnprivatizeAdminError::PgpImportOrDerive(e.to_string()))
}

/// Hex SHA-256 of the org public subkey.
fn pgp_hex_org_fingerprint<P: PGPProviderSync>(
    pgp: &P,
    org_private: &P::PrivateKey,
) -> Result<String, UnprivatizeAdminError> {
    let org_public = pgp
        .private_key_to_public_key(org_private)
        .map_err(|e| UnprivatizeAdminError::PgpImportOrDerive(e.to_string()))?;
    org_public
        .sha256_key_fingerprints()
        .first()
        .ok_or(UnprivatizeAdminError::NoOrgSha256Fingerprint)
        .map(|fp| fp.to_string())
}

fn pick_primary_address_unlocked_key<P: PGPProviderSync>(
    pgp: &P,
    addrs: &LtCoreGetAddressesRes,
    user_keys: &[UnlockedUserKey<P>],
    key_passphrase: &KeySecret,
) -> Result<(LtAuthAddressId, UnlockedAddressKey<P>), UnprivatizeAdminError> {
    let primary_address = addrs
        .addresses
        .iter()
        .min_by_key(|a| a.order)
        .ok_or(UnprivatizeAdminError::NoAddresses)?;
    let addr_id = primary_address.id.clone();
    let addr_unlock = primary_address
        .keys
        .0
        .unlock(pgp, user_keys, Some(key_passphrase));
    let primary = addr_unlock
        .unlocked_keys
        .into_iter()
        .find(|k| k.primary)
        .ok_or(UnprivatizeAdminError::NoPrimaryAddressKey)?;
    Ok((addr_id, primary))
}

fn pgp_sign_fingerprint_utc0<P: PGPProviderSync>(
    pgp: &P,
    address_private: &P::PrivateKey,
    data: &[u8],
) -> Result<String, UnprivatizeAdminError> {
    let ctx = pgp.new_signing_context(ORG_FINGERPRINT_CONTEXT.to_owned(), true);
    let bytes = pgp
        .new_signer()
        .with_signing_key(address_private)
        .with_signing_context(&ctx)
        .at_signing_time(UnixTimestamp::zero())
        .sign_detached(data, DataEncoding::Armor)
        .map_err(|e| UnprivatizeAdminError::PgpSignFingerprint(e.to_string()))?;
    String::from_utf8(bytes).map_err(UnprivatizeAdminError::PgpArmoredNotUtf8)
}

fn pgp_sign_invitation<P: PGPProviderSync>(
    pgp: &P,
    org_private: &P::PrivateKey,
    data: &[u8],
) -> Result<String, UnprivatizeAdminError> {
    let ctx = pgp.new_signing_context(UNPRIVATIZATION_INVITE_CONTEXT.to_owned(), true);
    let bytes = pgp
        .new_signer()
        .with_signing_key(org_private)
        .with_signing_context(&ctx)
        .sign_detached(data, DataEncoding::Armor)
        .map_err(|e| UnprivatizeAdminError::PgpSignInvitation(e.to_string()))?;
    String::from_utf8(bytes).map_err(UnprivatizeAdminError::PgpArmoredNotUtf8)
}

fn build_invitation_json_string(member_email: &str) -> String {
    format!(r#"{{"Address":"{member_email}", "Revision":1, "Admin":false}}"#)
}

fn find_member_by_name<'a>(
    res: &'a LtCoreGetMembersRes,
    email: &str,
) -> Result<&'a LtCoreMemberInfo, UnprivatizeAdminError> {
    res.members.iter().find(|m| m.name == email).ok_or_else(|| {
        UnprivatizeAdminError::MemberNotFound {
            email: email.to_string(),
            num_members: res.members.len(),
        }
    })
}

// --- public API ---------------------------------------------------------------

/// `GET` users, salts, unlock user keys, import org private key.
pub async fn load_admin_pgp_state<P: PGPProviderSync>(
    pgp: P,
    session: &Session,
    admin_password: &str,
) -> Result<AdminPgpState<P>, UnprivatizeAdminError> {
    let user_res = core_get_users(session).await?;
    let admin_user = user_res.user;
    let primary_id = find_primary_key_id(&admin_user)?;
    let key_salts = core_get_key_salts(session).await?.key_salts;
    let key_passphrase = derive_key_passphrase(&key_salts, &primary_id, admin_password)?;
    let user_keys = unlock_user_user_keys(&pgp, &admin_user, &key_passphrase)?;
    let org_res = core_get_org_keys(session).await?;
    let org_armor = org_res
        .private_key
        .as_ref()
        .ok_or(UnprivatizeAdminError::NoOrgPrivateKey)?;
    let org_private = pgp_import_org_private_key(&pgp, org_armor.as_str(), &key_passphrase)?;
    Ok(AdminPgpState {
        pgp,
        key_passphrase,
        user_keys,
        org_private,
    })
}

/// `PUT /core/v4/organizations/keys/signature` (publishes org “identity” over the org key fingerprint).
pub async fn publish_org_identity<P: PGPProviderSync>(
    session: &Session,
    state: &AdminPgpState<P>,
) -> Result<(), UnprivatizeAdminError> {
    let pgp = &state.pgp;
    let fp_hex = pgp_hex_org_fingerprint(pgp, &state.org_private)?;
    let addrs = core_get_addresses(session).await?;
    let (address_id, primary_addr) =
        pick_primary_address_unlocked_key(pgp, &addrs, &state.user_keys, &state.key_passphrase)?;
    let armored = pgp_sign_fingerprint_utc0(pgp, &primary_addr.private_key, fp_hex.as_bytes())?;
    let _: lattice::core::put_organizations_keys_signature::LtCorePutOrganizationsKeysSignatureRes =
        session
            .send_lt(LtCorePutOrganizationsKeysSignatureReq {
                body: LtCorePutOrganizationsKeysSignatureBody {
                    signature: Sensitive::new(armored),
                    address_id,
                },
            })
            .await
            .map_err(UnprivatizeAdminError::from)?;
    Ok(())
}

/// `POST /core/v4/members/{id}/unprivatize`.
pub async fn unprivatize_member<P: PGPProviderSync>(
    session: &Session,
    state: &AdminPgpState<P>,
    member_email: &str,
) -> Result<(), UnprivatizeAdminError> {
    let pgp = &state.pgp;
    let members = core_get_members(session).await?;
    let m = find_member_by_name(&members, member_email)?;
    let invitation_data = build_invitation_json_string(member_email);
    let inv_sig = pgp_sign_invitation(pgp, &state.org_private, invitation_data.as_bytes())?;
    let _: lattice::core::post_members_unprivatize::LtCorePostMembersUnprivatizeRes = session
        .send_lt(LtCorePostMembersUnprivatizeReq {
            member_id: m.id.clone(),
            body: LtCorePostMembersUnprivatizeBody {
                invitation_data: LtCoreUnprivInvitationData(invitation_data),
                invitation_signature: LtCoreUnprivInvitationSignature(Sensitive::new(inv_sig)),
            },
        })
        .await
        .map_err(UnprivatizeAdminError::from)?;
    Ok(())
}

/// `load` → `publish` → `unprivatize` in one go.
pub async fn admin_publish_org_identity_and_unprivatize_member(
    session: &Session,
    admin_password: &str,
    member_email: &str,
) -> Result<(), UnprivatizeAdminError> {
    let pgp = new_pgp_provider();
    let state = load_admin_pgp_state(pgp, session, admin_password).await?;
    publish_org_identity(session, &state).await?;
    unprivatize_member(session, &state, member_email).await
}
