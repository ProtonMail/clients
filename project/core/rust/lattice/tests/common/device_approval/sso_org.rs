use proton_crypto::crypto::PGPProviderSync;
use proton_crypto::new_pgp_provider;

use super::super::{
    Session, generate_muon_session, login_muon_session, random_password, random_string, sso_login,
    sso_member_setup, sso_setup, unprivatize_admin::AdminPgpState,
};
use super::error::DeviceApprovalError;
use super::pending_device::PendingDevice;
use super::unprivatized_member::UnprivatizedMember;

pub struct SsoOrg {
    pub admin_session: Session,
    pub admin_password: String,
    pub domain_name: String,
}

impl SsoOrg {
    pub async fn load_admin_pgp_state(
        &self,
    ) -> Result<AdminPgpState<impl PGPProviderSync>, DeviceApprovalError> {
        let pgp = new_pgp_provider();
        AdminPgpState::load(pgp, &self.admin_session, &self.admin_password)
            .await
            .map_err(Into::into)
    }

    pub async fn publish_org_identity<P: PGPProviderSync>(
        &self,
        admin_state: &AdminPgpState<P>,
    ) -> Result<(), DeviceApprovalError> {
        admin_state
            .publish_org_identity(&self.admin_session)
            .await
            .map_err(Into::into)
    }

    pub async fn unprivatize_member<P: PGPProviderSync>(
        &self,
        admin_state: &AdminPgpState<P>,
        member_email: &str,
    ) -> Result<(), DeviceApprovalError> {
        admin_state
            .unprivatize_member(&self.admin_session, member_email)
            .await
            .map_err(Into::into)
    }

    /// Load admin PGP state once, publish org identity, then invite. Prefer the split
    /// [`Self::publish_org_identity`] / [`Self::unprivatize_member`] methods when tests
    /// need to assert server state between those steps.
    pub async fn invite_member(&self, member_email: &str) -> Result<(), DeviceApprovalError> {
        let admin_state = self.load_admin_pgp_state().await?;
        self.publish_org_identity(&admin_state).await?;
        self.unprivatize_member(&admin_state, member_email).await?;
        Ok(())
    }

    /// Atlas org + SSO domain setup. Quark helpers in [`sso_setup`] panic on failure.
    pub async fn bootstrap() -> Result<Self, DeviceApprovalError> {
        let session_init = generate_muon_session().await;
        let admin_username = format!("ssoa_{}", random_string(8));
        let admin_password = random_password();

        let admin_user =
            sso_setup::purchase_pass_business_plan(&session_init, &admin_username, &admin_password)
                .await;
        let admin_user_id = admin_user.user_id;
        let org_res =
            sso_setup::create_organization(&session_init, admin_user_id, &admin_password).await;

        let suffix = random_string(6).to_lowercase();
        let domain_name = format!("d{suffix}.protonhub.org");

        let (admin_session, _) =
            login_muon_session(session_init, &admin_username, &admin_password).await?;

        sso_setup::setup_org_sso_domain(&admin_session, &domain_name, org_res.organization_id)
            .await;

        Ok(Self {
            admin_session,
            admin_password,
            domain_name,
        })
    }

    pub async fn provision_unprivatized_member(
        &self,
    ) -> Result<UnprivatizedMember, DeviceApprovalError> {
        let member_local = format!("ssou_{}", random_string(8));
        let member_email = format!("{member_local}@{}", self.domain_name);
        let backup_password = random_password();

        let _ = sso_login::login_with_sso(generate_muon_session().await, &member_email).await?;

        let admin_state = self.load_admin_pgp_state().await?;
        self.publish_org_identity(&admin_state).await?;
        self.unprivatize_member(&admin_state, &member_email).await?;

        let member_session =
            sso_login::login_with_sso(generate_muon_session().await, &member_email).await?;

        sso_member_setup::setup_sso_member_keys(&member_session, &backup_password).await?;

        let org_passphrase = admin_state
            .complete_member_keys_unprivatization(&self.admin_session, &member_email)
            .await?;

        Ok(UnprivatizedMember {
            email: member_email,
            backup_password,
            org_passphrase,
            session: member_session,
        })
    }

    pub async fn complete_admin_device(
        &self,
        member: &UnprivatizedMember,
        name: &str,
    ) -> Result<PendingDevice, DeviceApprovalError> {
        let pending = PendingDevice::register(&member.session, name).await?;
        pending.request_admin_activation(&member.session).await?;

        let admin_state = self.load_admin_pgp_state().await?;
        let pending_row = pending.fetch_admin_pending_row(&self.admin_session).await?;
        super::approve::approve_member_device(
            &admin_state,
            &self.admin_session,
            &member.email,
            Some(&member.org_passphrase),
            &pending_row,
            &pending.confirmation_code,
        )
        .await?;

        pending
            .expect_absent_from_admin_pending(&self.admin_session)
            .await?;
        pending.associate(&member.session).await?;
        Ok(pending)
    }
}
