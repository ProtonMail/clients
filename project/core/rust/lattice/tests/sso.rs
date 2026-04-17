mod common;
mod common_sso;

use std::num::NonZeroU32;

use lattice::{
    LtApiResponseError, LtApiResponseErrorInfo,
    auth::{LtAuthPasswordMode, LtAuthTwoFactorMethod},
    core::{
        LtCoreDomainId, LtCoreDomainVerifyState, LtCoreSsoType,
        get_domain::{LtCoreGetDomainReq, LtCoreGetDomainRes},
        get_domains::{LtCoreGetDomainsReq, LtCoreGetDomainsRes},
        get_members::{LtCoreGetMembersReq, LtCoreGetMembersRes},
        post_domains::LtCoreDomainOutput,
        post_saml_setup_fields::{LtCorePostSamlSetupFieldsReq, LtCorePostSamlSetupFieldsRes},
        user_settings::{LtCoreGetSettingsReq, LtCoreGetSettingsRes},
    },
    quark::{
        payments::subscribed_user_seed::{
            LtQuarkNewPaymentsSeedSubscribedUser, LtQuarkNewPaymentsSeedSubscribedUserRes,
        },
        user::{
            domain_create::{LtQuarkOrganizationCreateDomain, LtQuarkOrganizationCreateDomainRes},
            organization_create::{
                LtQuarkUserCreateOrganization, LtQuarkUserCreateOrganizationRes,
            },
        },
    },
};

use crate::common::{Session, login_muon_session, random_string};

async fn purchase_pass_business_plan(
    session: &Session,
    username: &str,
    password: &str,
) -> LtQuarkNewPaymentsSeedSubscribedUserRes {
    session
        .send_quark(LtQuarkNewPaymentsSeedSubscribedUser {
            username: username.to_string(),
            password: password.to_string(),
            plan: Some(r#"{"passbiz2024":1,"1member-passbiz2024":9}"#.to_string()),
            currency: Some("EUR".to_string()),
            cycle: Some("12".to_string()),
            ..Default::default()
        })
        .await
        .unwrap()
}

async fn create_organization(
    session: &Session,
    user_id: u64,
    password: &str,
) -> LtQuarkUserCreateOrganizationRes {
    session
        .send_quark(LtQuarkUserCreateOrganization {
            user_id,
            password: password.to_string(),
            ..Default::default()
        })
        .await
        .unwrap()
        .0
}

async fn get_domains_lt(session: &Session) -> LtCoreGetDomainsRes {
    session
        .send_lt(LtCoreGetDomainsReq {
            page_size: Some(NonZeroU32::new(150).expect("150 is valid page size")),
            page: Some(0),
        })
        .await
        .unwrap()
}

async fn create_domain_quark(
    session: &Session,
    domain_name: &str,
    organization_id: u64,
) -> LtQuarkOrganizationCreateDomainRes {
    session
        .send_quark(LtQuarkOrganizationCreateDomain {
            organization_id,
            domain_name: Some(domain_name.to_string()),
            // flags: Some(2),
            ..Default::default()
        })
        .await
        .unwrap()
}

async fn get_domain_by_id_lt(session: &Session, domain_id: &LtCoreDomainId) -> LtCoreGetDomainRes {
    session
        .send_lt(LtCoreGetDomainReq {
            domain_id: domain_id.clone(),
            refresh: Some(true),
        })
        .await
        .unwrap()
}

async fn set_sso_domain(
    session: &Session,
    domain_id: &LtCoreDomainId,
) -> LtCorePostSamlSetupFieldsRes {
    session
        .send_lt(LtCorePostSamlSetupFieldsReq {
            domain_id: domain_id.clone(),
            sso_url: "https://sso.protonauth.com/sso/saml".to_string(),
            sso_entity_id: "https://sso.protonauth.com/identifier".to_string(),
            certificate: include_str!("sso_cert.pem").to_string(),
            saml_type: LtCoreSsoType::Default,
        })
        .await
        .unwrap()
}

async fn get_domain_lt(session: &Session, domain_name: &str) -> LtCoreDomainOutput {
    let domains = get_domains_lt(session).await;
    domains
        .domains
        .into_iter()
        .find(|d| d.domain_name == domain_name)
        .unwrap()
}

async fn get_members(session: &Session) -> LtCoreGetMembersRes {
    session.send_lt(LtCoreGetMembersReq).await.unwrap()
}

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

    let admin_user = purchase_pass_business_plan(&session_init, &username, &password).await;

    let org_res = create_organization(&session_init, admin_user.user_id, &password).await;

    let random_suffix = random_string(6).to_lowercase();

    let domain_name = format!("d{random_suffix}.protonhub.org");

    let (session_init, _) = login_muon_session(session_init, &username, &password)
        .await
        .unwrap();

    let domain = create_domain_quark(&session_init, &domain_name, org_res.organization_id).await;

    assert_eq!(
        domain.organization_id, org_res.organization_id,
        "Domain organization ID is not correct"
    );
    assert_eq!(
        domain.domain_name, domain_name,
        "Domain name is not correct"
    );

    let domain_lt = get_domain_lt(&session_init, &domain_name).await;

    assert_eq!(
        domain_lt.verify_state,
        LtCoreDomainVerifyState::Default,
        "Domain verify state is not Default (0)"
    );
    assert_eq!(
        domain_lt.domain_name, domain_name,
        "Domain name is not correct"
    );

    let sso_fields = set_sso_domain(&session_init, &domain_lt.id).await;
    assert!(sso_fields.sso.enabled, "SSO is not enabled");
    assert_eq!(
        sso_fields.sso.saml_type,
        LtCoreSsoType::Default,
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
    let domain_lt = get_domain_by_id_lt(&session_init, &domain_lt.id).await;
    assert_eq!(
        domain_lt.domain.verify_state,
        LtCoreDomainVerifyState::Good,
        "Domain verify state is not Good (2)"
    );

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
    let users = get_members(&session_init).await;
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
