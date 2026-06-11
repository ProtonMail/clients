//! Shared atlas setup for SSO + org tests (Pass Business admin, domain, SAML IdP fields).

use lattice::core::get_domain::{LtCoreGetDomainReq, LtCoreGetDomainRes};
use lattice::core::get_domains::LtCoreGetDomainsReq;
use lattice::core::post_domains::{LtCoreDomainOutput, LtCorePostDomainsReq};
use lattice::core::post_saml_setup_fields::{
    LtCorePostSamlSetupFieldsReq, LtCorePostSamlSetupFieldsRes,
};
use lattice::core::put_domain_flags::LtCorePutDomainFlagsReq;
use lattice::core::user_settings::{LtCoreGetSettingsReq, LtCoreGetSettingsRes};
use lattice::core::{LtCoreDomainId, LtCoreDomainVerifyState, LtCoreSsoType};
use lattice_quark::payments::subscribed_user_seed::{
    LtQuarkNewPaymentsSeedSubscribedUser, LtQuarkNewPaymentsSeedSubscribedUserRes,
};
use lattice_quark::user::domain_create::{
    LtQuarkOrganizationCreateDomain, LtQuarkOrganizationCreateDomainRes,
};
use lattice_quark::user::organization_create::{
    LtQuarkUserCreateOrganization, LtQuarkUserCreateOrganizationRes,
};

use super::Session;

/// Pass Business plan with multiple seats (see `tests/organization.rs` for a single-seat plan).
pub async fn purchase_pass_business_plan(
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

pub async fn create_organization(
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

async fn get_domains_lt(session: &Session) -> Vec<LtCoreDomainOutput> {
    session.fetch_all_pages(LtCoreGetDomainsReq).await.unwrap()
}

pub async fn create_domain_quark(
    session: &Session,
    domain_name: &str,
    organization_id: u64,
) -> LtQuarkOrganizationCreateDomainRes {
    session
        .send_quark(LtQuarkOrganizationCreateDomain {
            organization_id,
            domain_name: Some(domain_name.to_string()),
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

pub async fn set_domain_allowed_for_sso(session: &Session, domain_id: &LtCoreDomainId) {
    let result = session
        .send_lt(LtCorePutDomainFlagsReq {
            domain_id: domain_id.clone(),
            allowed_for_mail: None,
            allowed_for_sso: Some(true),
        })
        .await;
    assert!(
        result.is_ok(),
        "PUT /core/v4/domains/{domain_id}/flags with AllowedForSSO failed: {result:?}"
    );
}

/// Create org domain (POST with SSO intent, or Quark). Does not configure SAML or flags after
/// activation — use [`setup_org_sso_domain`] for the full harness sequence.
pub async fn create_org_sso_domain(
    session: &Session,
    domain_name: &str,
    organization_id: u64,
) -> LtCoreDomainOutput {
    if let Ok(res) = session
        .send_lt(LtCorePostDomainsReq {
            name: domain_name.to_string(),
            allowed_for_mail: Some(false),
            allowed_for_sso: Some(true),
        })
        .await
    {
        return res.domain;
    }

    create_domain_quark(session, domain_name, organization_id).await;
    get_domain_lt(session, domain_name).await
}

pub async fn set_sso_domain(
    session: &Session,
    domain_id: &LtCoreDomainId,
) -> LtCorePostSamlSetupFieldsRes {
    session
        .send_lt(LtCorePostSamlSetupFieldsReq {
            domain_id: domain_id.clone(),
            sso_url: "https://sso.protonauth.com/sso/saml".to_string(),
            sso_entity_id: "https://sso.protonauth.com/identifier".to_string(),
            certificate: include_str!("../sso_cert.pem").to_string(),
            saml_type: LtCoreSsoType::Default,
        })
        .await
        .unwrap()
}

pub async fn get_domain_lt(session: &Session, domain_name: &str) -> LtCoreDomainOutput {
    let domains = get_domains_lt(session).await;
    domains
        .into_iter()
        .find(|d| d.domain_name == domain_name)
        .unwrap()
}

/// Refresh domain after SAML setup so `verify_state` is **Good** for SSO.
pub async fn refresh_domain_good(
    session: &Session,
    domain_id: &LtCoreDomainId,
) -> LtCoreGetDomainRes {
    get_domain_by_id_lt(session, domain_id).await
}

/// After SAML + verify Good: set `DomainFlags::Sso` via `PUT …/flags` if needed, then assert
/// `sso-intent`.
pub async fn ensure_domain_sso_intent(session: &Session, domain_id: &LtCoreDomainId) {
    let mut refreshed = refresh_domain_good(session, domain_id).await;
    assert_domain_verify_good(&refreshed);

    if !refreshed.domain.flags.sso_intent {
        set_domain_allowed_for_sso(session, domain_id).await;
        refreshed = refresh_domain_good(session, domain_id).await;
    }

    assert!(
        refreshed.domain.flags.sso_intent,
        "expected Flags.sso-intent after AllowedForSSO + SAML; domain={:?}",
        refreshed.domain.domain_name
    );
}

/// Create domain, SAML IdP fields, DNS verify Good, and `sso-intent` flag.
pub async fn setup_org_sso_domain(
    session: &Session,
    domain_name: &str,
    organization_id: u64,
) -> LtCoreDomainOutput {
    let domain = create_org_sso_domain(session, domain_name, organization_id).await;
    set_sso_domain(session, &domain.id).await;
    ensure_domain_sso_intent(session, &domain.id).await;
    refresh_domain_good(session, &domain.id).await.domain
}

pub async fn get_user_settings(session: &Session) -> LtCoreGetSettingsRes {
    session.send_lt(LtCoreGetSettingsReq).await.unwrap()
}

/// After [`set_sso_domain`], re-fetch and assert the domain is in **Good** verify state.
pub fn assert_domain_verify_good(domain: &lattice::core::get_domain::LtCoreGetDomainRes) {
    assert_eq!(
        domain.domain.verify_state,
        LtCoreDomainVerifyState::Good,
        "Domain verify state is not Good (2)"
    );
}
