//! Shared atlas setup for SSO + org tests (Pass Business admin, domain, SAML IdP fields).

use std::num::NonZeroU32;

use lattice::LtSlimApiPageQuery;
use lattice::core::get_domain::{LtCoreGetDomainReq, LtCoreGetDomainRes};
use lattice::core::get_domains::{LtCoreGetDomainsReq, LtCoreGetDomainsRes, MAX_PAGE_SIZE};
use lattice::core::get_members::{LtCoreGetMembersReq, LtCoreGetMembersRes};
use lattice::core::post_domains::LtCoreDomainOutput;
use lattice::core::post_saml_setup_fields::{
    LtCorePostSamlSetupFieldsReq, LtCorePostSamlSetupFieldsRes,
};
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

async fn get_domains_lt(session: &Session) -> LtCoreGetDomainsRes {
    let page_size = NonZeroU32::new(MAX_PAGE_SIZE).expect("valid page size");
    session
        .send_lt(LtCoreGetDomainsReq {
            pagination: LtSlimApiPageQuery::new()
                .with_pagination(Some(0), Some(page_size))
                .expect("MAX_PAGE_SIZE is valid page size"),
        })
        .await
        .unwrap()
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
        .domains
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

pub async fn get_members(session: &Session) -> LtCoreGetMembersRes {
    session
        .send_lt(LtCoreGetMembersReq::default())
        .await
        .unwrap()
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
