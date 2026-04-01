use mail_common::models::MailSettings;
use mail_common::test_utils::init::Params as TestParams;
use mail_common::test_utils::test_context::MailTestContext;
use mail_core_api::services::proton::PrivateEmailRef;
use mail_core_common::services::crypto_key_service::core_key_manager::{
    PublicAddressKeyApiFetchPolicy, PublicAddressKeyContactFetchPolicy,
};
use mail_crypto_inbox::keys::PackageCryptoType;
use mail_crypto_inbox::message::packages::PackageMimeType;
use mail_crypto_inbox::proton_crypto;

#[tokio::test]
async fn load_sending_preferences() {
    let ctx = MailTestContext::new().await;
    let params = TestParams::default_basic();
    ctx.setup_user(params.clone()).await;
    let user_ctx = ctx.mail_user_context().await;

    let pgp = proton_crypto::new_pgp_provider();

    let recipient_email = params
        .recipient_keys
        .first()
        .expect("no test recipient found")
        .0
        .as_str();

    let tether = user_ctx.user_stash().connection().await.unwrap();

    let mail_settings = MailSettings::get(&tether)
        .await
        .expect("Failed to get mail settings")
        .unwrap();

    let recipient_preferences = user_ctx
        .recipient_send_preferences(
            &pgp,
            &tether,
            PrivateEmailRef::new(recipient_email),
            mail_settings.crypto_mail_settings(),
            Default::default(),
            PublicAddressKeyApiFetchPolicy::RequireSync,
            PublicAddressKeyContactFetchPolicy::RequireSync,
        )
        .await
        .unwrap();

    assert!(recipient_preferences.encrypt);
    assert!(recipient_preferences.sign);
    assert_eq!(
        recipient_preferences.pgp_scheme,
        PackageCryptoType::ProtonMail
    );
    assert_eq!(recipient_preferences.mime_type, PackageMimeType::Html);
    assert!(!recipient_preferences.encryption_disabled);
    assert!(!recipient_preferences.is_selected_key_pinned);
    assert!(recipient_preferences.selected_key.is_some());
}

#[tokio::test]
async fn load_sending_preferences_for_self() {
    let ctx = MailTestContext::new().await;
    let params = TestParams::default_basic();
    ctx.setup_user(params.clone()).await;
    let user_ctx = ctx.mail_user_context().await;

    let pgp = proton_crypto::new_pgp_provider();
    let self_address = params.addresses.first().unwrap().email.as_str();
    let tether = user_ctx.user_stash().connection().await.unwrap();

    let mail_settings = MailSettings::get(&tether)
        .await
        .expect("Failed to get mail settings")
        .unwrap();

    let recipient_preferences = user_ctx
        .recipient_send_preferences(
            &pgp,
            &tether,
            PrivateEmailRef::new(self_address),
            mail_settings.crypto_mail_settings(),
            Default::default(),
            PublicAddressKeyApiFetchPolicy::RequireSync,
            PublicAddressKeyContactFetchPolicy::RequireSync,
        )
        .await
        .unwrap();

    assert!(recipient_preferences.encrypt);
    assert!(recipient_preferences.sign);
    assert_eq!(
        recipient_preferences.pgp_scheme,
        PackageCryptoType::ProtonMail
    );
    assert_eq!(recipient_preferences.mime_type, PackageMimeType::Html);
    assert!(!recipient_preferences.encryption_disabled);
    assert!(!recipient_preferences.is_selected_key_pinned);
    assert!(recipient_preferences.selected_key.is_some());
}
