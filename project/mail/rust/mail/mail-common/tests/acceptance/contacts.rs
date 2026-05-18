use mail_common::test_utils::init::Params as TestParams;
use mail_common::test_utils::test_context::{MailTestContext, MailUserContextTestExtension};
use mail_contacts_api::mocks::ContactsMockServerExt;
use mail_core_api::services::proton::{
    ContactBasic as ApiContactBasic, ContactEmail as ApiContactEmail, ContactId,
    ContactSendingPreferences,
};
use mail_core_common::models::{Contact, ModelIdExtension, action_delete_contacts};
use mail_stash::orm::Model;

// This test needs to remain here as it depends on actions
#[tokio::test]
async fn delete_contacts() {
    let ctx = MailTestContext::new().await;
    let mut params = TestParams::default_basic();

    params.contacts = vec![ApiContactBasic {
        id: "123".into(),
        create_time: 0,
        label_ids: vec![],
        modify_time: 0,
        name: "Mr Banksy".to_string(),
        size: 0,
        uid: "123".into(),
    }];

    params.emails = vec![ApiContactEmail {
        id: "321".into(),
        contact_id: "123".into(),
        canonical_email: "".into(),
        contact_type: vec![],
        defaults: ContactSendingPreferences::Default,
        email: "banksy@proton.me".into(),
        is_proton: true,
        label_ids: vec![],
        last_used_time: 0,
        name: "Mr Banksy".to_string(),
        order: 0,
    }];

    ctx.setup_user(params.clone()).await;
    ctx.mock_server()
        .mock_delete_contacts(vec!["123".into()])
        .await;

    // Initialize Mocking
    let user_ctx = ctx.mail_user_context().await;
    let tether = user_ctx.user_stash().connection();

    let contact = Contact::find_by_remote_id(ContactId::from("123"), &tether)
        .await
        .unwrap()
        .unwrap();

    assert!(!contact.deleted);

    action_delete_contacts(user_ctx.action_queue(), vec![contact.id()])
        .await
        .unwrap();
    user_ctx.execute_single_action().await.unwrap();

    let contact = Contact::find_by_remote_id(ContactId::from("123"), &tether)
        .await
        .unwrap()
        .unwrap();

    assert!(contact.deleted);

    let contact_list = Contact::contact_list(&tether).await.unwrap();
    assert_eq!(contact_list.len(), 0);
}
