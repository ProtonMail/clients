// Lattice tests chain `Session` → `Muon2Transport` → `LtContract` over muon's `GenericContext`
// (connector, store, CookieStore). Muon 2.4+ deepens those nested generics; rustc's default
// `recursion_limit` (128) is exceeded when computing async fn layout (e.g. unprivatize helpers).
#![recursion_limit = "256"]

//! Integration tests for org unprivatization:
//! * admin `GET /organizations/keys` (after identity publish), `PUT /organizations/keys/signature`,
//!   `POST /members/{id}/unprivatize`, `GET /members` (list row embed), member-device admin routes
//!   `GET /members/{id}/devices`, `GET /members/devices/pending`, `GET /members/{id}/addresses`,
//!   member `GET /auth/v4/devices`
//! * member `GET /members/me/unprivatize` (**Pending** after a valid invite)
//!
//! Requires a live atlas stack; run with `ENV_NAME=davy` (see `tests/common/muon.rs`).

mod common;

use lattice::auth::devices::get_auth_devices::LtAuthGetDevicesReq;
use lattice::core::get_members::LtCoreGetMembersReq;
use lattice::core::get_members_me_unprivatize::LtCoreGetMembersMeUnprivatizeReq;
use lattice::core::get_organizations_keys::LtCoreGetOrganizationsKeysReq;
use lattice::core::ids::LtCoreMemberEncId;
use lattice::core::members::addresses::LtCoreGetMembersMemberIDAddressesReq;
use lattice::core::members::devices::{
    LtCoreGetMembersDevicesPendingReq, LtCoreGetMembersDevicesReq,
};
use lattice::core::unpriv_types::LtCoreUnprivState;
use lattice::core::user_settings::LtCoreGetSettingsReq;
use lattice::{LtApiResponseError, LtApiResponseErrorInfo};

use crate::common::{
    device_approval::sso_org::SsoOrg, generate_muon_session, random_string, sso_login,
};

/// Same unauthenticated preflight as `tests/sso.rs`.
#[tokio::test]
async fn test_unprivatize_admin_sets_member_me_endpoint_to_pending() {
    let session_init = generate_muon_session().await;

    let res = session_init.send_lt(LtCoreGetSettingsReq).await;
    assert_api_err!(
        res,
        LtApiResponseError::AccessTokenWithInsufficientScope(LtApiResponseErrorInfo { .. })
    );

    let org = SsoOrg::bootstrap().await.expect("bootstrap sso org");
    let subuser_with_domain = format!("ssou_{}@{}", random_string(8), org.domain_name);

    // `POST /auth` (SSO) must run first so the org member row exists.
    let subuser_after_sso =
        sso_login::login_with_sso(generate_muon_session().await, &subuser_with_domain)
            .await
            .expect("SSO subuser seed");

    let me_before = subuser_after_sso
        .send_lt(LtCoreGetMembersMeUnprivatizeReq)
        .await;
    assert_api_err!(me_before, LtApiResponseError::UnprivatizationNotExists(..));

    // One PGP load, then publish before unprivatize (reuses `AdminPgpState`).
    let admin_state = org
        .load_admin_pgp_state()
        .await
        .expect("load admin PGP state");
    org.publish_org_identity(&admin_state)
        .await
        .expect("PUT /organizations/keys/signature");

    let org_keys_after_publish = org
        .admin_session
        .send_lt(LtCoreGetOrganizationsKeysReq)
        .await
        .expect("GET /organizations/keys after identity publish");
    assert!(
        org_keys_after_publish
            .public_key
            .as_ref()
            .is_some_and(|k| !k.is_empty()),
        "GET /organizations/keys should return org public key for admin (got: public_key={:?})",
        org_keys_after_publish.public_key.as_ref().map(|k| k.len())
    );
    let fp = org_keys_after_publish
        .fingerprint_signature
        .as_ref()
        .expect("fingerprint signature after publish");
    assert!(
        fp.as_str().contains("-----BEGIN PGP SIGNATURE-----"),
        "fingerprint signature should be armored PGP"
    );

    org.unprivatize_member(&admin_state, &subuser_with_domain)
        .await
        .expect("POST /members/…/unprivatize");

    let members = org
        .admin_session
        .fetch_all_pages(LtCoreGetMembersReq)
        .await
        .expect("GET /members");
    let sso_member = members
        .iter()
        .find(|m| m.name == subuser_with_domain)
        .expect("SSO user listed as org member");
    let list_unpriv = sso_member
        .unprivatization
        .as_ref()
        .expect("member list should include unprivatization after POST …/unprivatize");
    assert_eq!(list_unpriv.state, Some(LtCoreUnprivState::Pending));

    let sso_member_id = sso_member.id.clone();

    let member_addresses = org
        .admin_session
        .send_lt(LtCoreGetMembersMemberIDAddressesReq {
            member_id: sso_member_id.clone(),
            query: Default::default(),
        })
        .await
        .expect("GET /members/{id}/addresses");
    assert_eq!(member_addresses.addresses.len(), 1);

    let unknown_member_addresses = org
        .admin_session
        .send_lt(LtCoreGetMembersMemberIDAddressesReq {
            member_id: LtCoreMemberEncId("not-a-valid-member-id".to_string()),
            query: Default::default(),
        })
        .await;
    assert_api_err!(unknown_member_addresses, LtApiResponseError::InvalidID(..));

    let admin_member_devices = org
        .admin_session
        .send_lt(LtCoreGetMembersDevicesReq {
            member_id: sso_member_id,
        })
        .await
        .expect("GET /members/{id}/devices (admin)");

    let org_pending_devices = org
        .admin_session
        .send_lt(LtCoreGetMembersDevicesPendingReq)
        .await
        .expect("GET /members/devices/pending");
    assert!(org_pending_devices.member_auth_devices.is_empty());
    // Re-authenticate: access token from the pre-invite SSO can expire while admin crypto runs; muon may
    // then return `Auth(Session)` on the next `core` call.
    let subuser_session =
        sso_login::login_with_sso(generate_muon_session().await, &subuser_with_domain)
            .await
            .expect("SSO re-login for GET /members/me/unprivatize");

    let me = subuser_session
        .send_lt(LtCoreGetMembersMeUnprivatizeReq)
        .await
        .expect("GET /members/me/unprivatize");

    let my_devices = subuser_session
        .send_lt(LtAuthGetDevicesReq)
        .await
        .expect("GET /auth/v4/devices (member self)");

    assert!(
        admin_member_devices.auth_devices.is_empty(),
        "org-admin GET /members/{{id}}/devices should be empty"
    );
    assert!(
        my_devices.auth_devices.is_empty(),
        "member self GET /auth/v4/devices should be empty"
    );

    assert_eq!(me.state, LtCoreUnprivState::Pending);
    assert!(!me.admin_email.is_empty());
    let invite = me
        .invitation_data
        .as_ref()
        .expect("invitation_data should be set in Pending");
    assert!(
        invite.0.contains(&subuser_with_domain),
        "invitation_data should name the member email: {invite:?}"
    );

    assert_eq!(
        me.invitation_data, list_unpriv.invitation_data,
        "GET /members/me/unprivatize and GET /members: invitation_data"
    );
    assert_eq!(
        me.invitation_signature, list_unpriv.invitation_signature,
        "GET /members/me/unprivatize and GET /members: invitation_signature"
    );
    assert_eq!(
        me.invitation_email, list_unpriv.invitation_email,
        "GET /members/me/unprivatize and GET /members: invitation_email"
    );
    let sig = me
        .org_key_fingerprint_signature
        .expect("me.org_key_fingerprint_signature should be set");
    assert!(
        sig.0.contains("-----BEGIN PGP SIGNATURE-----"),
        "me.org_key_fingerprint_signature should be armored PGP if present"
    );

    let pk = me.org_public_key.expect("me.org_public_key should be set");
    assert!(
        !pk.0.is_empty() && pk.0.contains("BEGIN PGP"),
        "me.org_public_key should be armored org key if present: {pk:?}"
    );
}
