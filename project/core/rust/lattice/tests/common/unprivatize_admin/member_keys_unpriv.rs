use core_key::LtCoreMemberListUnprivatizationExt;
use lattice::core::post_members_keys_unprivatize::LtCorePostMembersKeysUnprivatizeReq;
use proton_crypto::crypto::PGPProviderSync;
use proton_crypto_account::salts::KeySecret;

use super::super::Session;
use super::admin_pgp_state::AdminPgpState;
use super::unprivatize_admin_error::UnprivatizeAdminError;

pub async fn complete_member_keys_unprivatization<P: PGPProviderSync>(
    session: &Session,
    member_email: &str,
    admin: &AdminPgpState<P>,
) -> Result<KeySecret, UnprivatizeAdminError> {
    let member = session.find_member_by_email(member_email).await?;
    let unpriv = member
        .unprivatization
        .as_ref()
        .filter(|u| u.ready_for_admin_keys_completion())
        .ok_or_else(|| UnprivatizeAdminError::UnprivatizationNotReady {
            email: member_email.to_string(),
        })?;

    let addrs = session.member_addresses(&member.id).await?;

    let (body, token) =
        admin
            .org_admin_pgp()
            .build_manual_unpriv_completion(member_email, unpriv, &addrs)?;

    session
        .send_lt(LtCorePostMembersKeysUnprivatizeReq {
            member_id: member.id,
            body,
        })
        .await?;
    Ok(token)
}
