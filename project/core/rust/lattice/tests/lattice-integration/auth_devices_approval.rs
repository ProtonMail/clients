use lattice::auth::devices::LtAuthDeviceState;

use crate::common::device_approval::sso_org::SsoOrg;

#[tokio::test]
async fn test_sso_member_user_then_admin_device_approval() {
    let org = SsoOrg::bootstrap().await.expect("bootstrap sso org");
    let member = org
        .provision_unprivatized_member_without_invitation_data()
        .await
        .expect("provision unprivatized sso member");

    let user = member
        .complete_user_device("lattice-compound-user")
        .await
        .expect("user self-approval");
    let admin = org
        .complete_admin_device(&member, "lattice-compound-admin")
        .await
        .expect("admin approval");

    admin
        .expect_state_on(&member.session, LtAuthDeviceState::Active)
        .await
        .expect("admin device active");

    let devices = member
        .session
        .auth_devices()
        .await
        .expect("list auth devices");

    let admin_device = devices
        .iter()
        .find(|d| d.id == admin.id)
        .unwrap_or_else(|| panic!("admin device {} not in member list", admin.id));
    assert_eq!(admin_device.state, LtAuthDeviceState::Active);

    if let Some(user_device) = devices.iter().find(|d| d.id == user.id) {
        assert_eq!(user_device.state, LtAuthDeviceState::Active);
    }

    user.expect_absent_from_admin_pending(&org.admin_session)
        .await
        .expect("user device not in admin pending");
    admin
        .expect_absent_from_admin_pending(&org.admin_session)
        .await
        .expect("admin device not in admin pending");
}
