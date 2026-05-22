// Lattice tests chain `Session` → `Muon2Transport` → `LtContract` over muon's `GenericContext`
// (connector, store, CookieStore). Muon 2.4+ deepens those nested generics; rustc's default
// `recursion_limit` (128) is exceeded when computing async fn layout (e.g. unprivatize helpers).
#![recursion_limit = "256"]

mod common;
mod common_sso;

use lattice::auth::{LtAuthPasswordMode, LtAuthTwoFactorMethod};
use lattice::core::user_settings::{LtCoreGetSettingsReq, LtCoreGetSettingsRes};
use lattice::{LtApiResponseError, LtApiResponseErrorInfo};

use crate::common::{Session, login_muon_session, random_string, sso_setup};

async fn get_user_settings(session: &Session) -> LtCoreGetSettingsRes {
    session.send_lt(LtCoreGetSettingsReq).await.unwrap()
}

#[tokio::test]
async fn test_sso_login_end_to_end() {
    let session_init = common::generate_muon_session().await;

    let res = session_init.send_lt(LtCoreGetSettingsReq).await;
    assert_api_err!(
        res,
        LtApiResponseError::AccessTokenWithInsufficientScope(LtApiResponseErrorInfo { .. })
    );

    let username = format!("ssoa_{}", random_string(8));
    let password = random_string(34);

    let admin_user =
        sso_setup::purchase_pass_business_plan(&session_init, &username, &password).await;

    let org_res =
        sso_setup::create_organization(&session_init, admin_user.user_id, &password).await;

    let random_suffix = random_string(6).to_lowercase();

    let domain_name = format!("d{random_suffix}.protonhub.org");

    let (session_init, _) = login_muon_session(session_init, &username, &password)
        .await
        .unwrap();

    let domain =
        sso_setup::create_domain_quark(&session_init, &domain_name, org_res.organization_id).await;

    assert_eq!(
        domain.organization_id, org_res.organization_id,
        "Domain organization ID is not correct"
    );
    assert_eq!(
        domain.domain_name, domain_name,
        "Domain name is not correct"
    );

    let domain_lt = sso_setup::get_domain_lt(&session_init, &domain_name).await;

    assert_eq!(
        domain_lt.verify_state,
        lattice::core::LtCoreDomainVerifyState::Default,
        "Domain verify state is not Default (0)"
    );
    assert_eq!(
        domain_lt.domain_name, domain_name,
        "Domain name is not correct"
    );

    let sso_fields = sso_setup::set_sso_domain(&session_init, &domain_lt.id).await;
    assert!(sso_fields.sso.enabled, "SSO is not enabled");
    assert_eq!(
        sso_fields.sso.saml_type,
        lattice::core::LtCoreSsoType::Default,
        "SAML type is not Default (1)"
    );
    assert_eq!(
        sso_fields.sso.sso_url, "https://sso.protonauth.com/sso/saml",
        "SSO URL is not correct"
    );
    assert_eq!(
        sso_fields.sso.sso_entity_id, "https://sso.protonauth.com/identifier",
        "SSO Entity ID is not correct"
    );
    assert_eq!(
        sso_fields.sso.certificate,
        include_str!("sso_cert.pem"),
        "Certificate is not correct"
    );

    // This is necessary to refresh the domain data after setting up SSO
    let domain_lt = sso_setup::refresh_domain_good(&session_init, &domain_lt.id).await;
    sso_setup::assert_domain_verify_good(&domain_lt);

    let subuser_username = format!("ssou_{}", random_string(8));

    let subuser_with_domain = format!("{}@{}", subuser_username, domain_name);

    let second_session = common::generate_muon_session().await;

    let second_session = common_sso::login_with_sso(second_session, &subuser_with_domain)
        .await
        .unwrap();

    // Checks the user settings
    let user_settings = get_user_settings(&second_session).await;

    assert_eq!(
        user_settings.user_settings.tfa.enabled,
        LtAuthTwoFactorMethod::default(),
        "TFA is enabled"
    );
    assert_eq!(
        user_settings.user_settings.tfa.allowed,
        LtAuthTwoFactorMethod::TOTP | LtAuthTwoFactorMethod::FIDO,
        "Some TFA methods are not allowed"
    );
    assert_eq!(
        user_settings.user_settings.password.mode,
        LtAuthPasswordMode::Two,
        "Password mode is not 2"
    );

    // Checks the members
    let users = sso_setup::get_members(&session_init).await;
    assert_eq!(
        users.members.len(),
        2,
        "Number of members is not 2. Users: {users:#?}"
    );
    let admin_user = users.members.iter().find(|m| m.name == username).unwrap();
    assert_eq!(
        admin_user.sso, 0,
        "SSO is enabled on the admin user: {admin_user:#?}"
    );
    let subuser_user = users
        .members
        .iter()
        .find(|m| m.name == subuser_with_domain)
        .unwrap();
    assert_eq!(
        subuser_user.sso, 1,
        "SSO is not enabled on the subuser: {subuser_user:#?}"
    );

    // Print the credentials of both users
    println!("Admin user credentials: {username} {password}");
    println!("Subuser user adress: {subuser_with_domain}");
}
