use std::sync::Arc;

use mail_action_queue::action::ActionGroup;
use mail_action_queue::rebase::RebaseChangeSet;
use mail_api_labels::LabelId;
use mail_common::MailUserContext;
use mail_common::actions::labels::{Create, Delete, Update};
use mail_common::test_utils::init::Params;
use mail_common::test_utils::test_context::{MailTestContext, MailUserContextTestExtension};
use mail_core_common::datatypes::{LabelColor, LabelType, WellKnownLabelColor};
use mail_core_common::models::Label;
use mail_stash::orm::Model;
use mail_stash::stash::Tether;

#[tokio::test]
async fn test_create_update_delete_custom_label() {
    let ctx = MailTestContext::new().await;
    let user_ctx = ctx.uninitialized_mail_user_context().await;
    let tether = user_ctx.user_stash().connection();

    ctx.setup_user(Params::default_basic()).await;
    ctx.initialize_uninitialized_ctx(&user_ctx).await;

    // Create label
    let name = "My Awesome Label";
    let color = WellKnownLabelColor::Pink;
    mail_api_labels::mocks::mock_create_label(
        name.into(),
        color.hex_code().to_owned(),
        Ok("label-1".into()),
    )
    .expect(1)
    .mount(ctx.mock_server())
    .await;
    let action = Create::new_custom_label(name.into(), color);
    user_ctx.action_queue().queue_action(action).await.unwrap();
    user_ctx.execute_single_action().await.unwrap();

    // Check label
    let label = Label::find_by_name_kind(name.to_owned(), LabelType::Label, &tether)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(label.color, LabelColor::from(color));
    assert!(label.notify);

    // Edit label
    let new_name = "My Better Label";
    let new_color = WellKnownLabelColor::Copper;
    mail_api_labels::mocks::mock_put_label(
        label.remote_id.clone().unwrap(),
        new_name.into(),
        new_color.hex_code().to_owned(),
        Ok(()),
    )
    .expect(1)
    .mount(ctx.mock_server())
    .await;
    let action = Update::new_custom_label(label.local_id.unwrap(), new_name.into(), new_color);
    user_ctx.action_queue().queue_action(action).await.unwrap();
    user_ctx.execute_single_action().await.unwrap();

    // Check label
    let label = Label::find_by_name_kind(new_name.to_owned(), LabelType::Label, &tether)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(label.color, LabelColor::from(new_color));

    // Delete label
    mail_api_labels::mocks::mock_delete_label(label.remote_id.clone().unwrap(), 200)
        .expect(1)
        .mount(ctx.mock_server())
        .await;
    let action = Delete::new(label.local_id.unwrap());
    user_ctx.action_queue().queue_action(action).await.unwrap();
    user_ctx.execute_single_action().await.unwrap();
    let labels = Label::find_by_kind(LabelType::Label, &tether)
        .await
        .unwrap();
    assert_eq!(labels.len(), 1);
}

#[tokio::test]
async fn test_create_update_delete_custom_folder() {
    let ctx = MailTestContext::new().await;
    let user_ctx = ctx.uninitialized_mail_user_context().await;
    let tether = user_ctx.user_stash().connection();

    ctx.setup_user(Params::default_basic()).await;
    ctx.initialize_uninitialized_ctx(&user_ctx).await;

    // Create folder
    let name = "My Awesome Folder";
    let color = WellKnownLabelColor::Reef;
    let notify = true;
    mail_api_labels::mocks::mock_create_folder(
        None,
        name.into(),
        color.hex_code().to_owned(),
        notify,
        Ok("folder-1".into()),
    )
    .expect(1)
    .mount(ctx.mock_server())
    .await;
    let action = Create::new_custom_folder(None, name.into(), color, true);
    user_ctx.action_queue().queue_action(action).await.unwrap();
    user_ctx.execute_single_action().await.unwrap();

    // Check folder
    let folder = Label::find_by_name_kind(name.to_owned(), LabelType::Folder, &tether)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(folder.local_parent_id, None);
    assert_eq!(folder.remote_parent_id, None);
    assert_eq!(folder.color, LabelColor::from(color));
    assert_eq!(folder.notify, notify);

    // Edit folder
    let new_name = "My Better Folder";
    let color = WellKnownLabelColor::Enzian;
    let notify = false;
    mail_api_labels::mocks::mock_put_folder(
        folder.remote_id.clone().unwrap(),
        None,
        new_name.into(),
        color.hex_code().to_owned(),
        notify,
        Ok(()),
    )
    .expect(1)
    .mount(ctx.mock_server())
    .await;
    let action = Update::new_custom_folder(
        folder.local_id.unwrap(),
        None,
        new_name.into(),
        color,
        notify,
    );
    user_ctx.action_queue().queue_action(action).await.unwrap();
    user_ctx.execute_single_action().await.unwrap();

    // Check folder
    let folder = Label::find_by_name_kind(new_name.to_owned(), LabelType::Folder, &tether)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(folder.local_parent_id, None);
    assert_eq!(folder.remote_parent_id, None);
    assert_eq!(folder.color, LabelColor::from(color));
    assert_eq!(folder.notify, notify);

    // Delete folder
    mail_api_labels::mocks::mock_delete_label(folder.remote_id.clone().unwrap(), 200)
        .expect(1)
        .mount(ctx.mock_server())
        .await;
    let action = Delete::new(folder.local_id.unwrap());
    user_ctx.action_queue().queue_action(action).await.unwrap();
    user_ctx.execute_single_action().await.unwrap();
    let folders = Label::find_by_kind(LabelType::Folder, &tether)
        .await
        .unwrap();
    assert_eq!(folders.len(), 0);
}

#[tokio::test]
async fn test_delete_custom_parent_folder() {
    let ctx = MailTestContext::new().await;
    let user_ctx = ctx.uninitialized_mail_user_context().await;
    let tether = user_ctx.user_stash().connection();

    ctx.setup_user(Params::default_basic()).await;
    ctx.initialize_uninitialized_ctx(&user_ctx).await;

    // Create parent folder
    let parent_name = "My Parent Folder";
    let parent_color = WellKnownLabelColor::Enzian;
    let parent_notify = true;
    mail_api_labels::mocks::mock_create_folder(
        None,
        parent_name.into(),
        parent_color.hex_code().to_owned(),
        parent_notify,
        Ok("folder-1".into()),
    )
    .expect(1)
    .mount(ctx.mock_server())
    .await;
    let action = Create::new_custom_folder(None, parent_name.into(), parent_color, true);
    user_ctx.action_queue().queue_action(action).await.unwrap();
    user_ctx.execute_single_action().await.unwrap();

    // Check parent folder
    let parent_folder =
        Label::find_by_name_kind(parent_name.to_owned(), LabelType::Folder, &tether)
            .await
            .unwrap()
            .unwrap();
    assert_eq!(parent_folder.local_parent_id, None);
    assert_eq!(parent_folder.remote_parent_id, None);
    assert_eq!(parent_folder.notify, parent_notify);

    // Create child folder
    let child_name = "My Child Folder";
    let child_color = WellKnownLabelColor::Reef;
    let child_notify = false;
    mail_api_labels::mocks::mock_create_folder(
        Some(parent_folder.remote_id.clone().unwrap()),
        child_name.into(),
        child_color.hex_code().to_owned(),
        child_notify,
        Ok("folder-2".into()),
    )
    .expect(1)
    .mount(ctx.mock_server())
    .await;
    let action = Create::new_custom_folder(
        Some(parent_folder.local_id.unwrap()),
        child_name.into(),
        child_color,
        child_notify,
    );
    user_ctx.action_queue().queue_action(action).await.unwrap();
    user_ctx.execute_single_action().await.unwrap();

    // Check child folder
    let child_folder = Label::find_by_name_kind(child_name.to_owned(), LabelType::Folder, &tether)
        .await
        .unwrap()
        .unwrap();
    assert_ne!(child_folder.local_id, parent_folder.local_id);
    assert_eq!(
        child_folder.local_parent_id,
        Some(parent_folder.local_id.unwrap())
    );
    assert_eq!(
        child_folder.remote_parent_id,
        Some(parent_folder.remote_id.clone().unwrap())
    );
    assert_eq!(child_folder.notify, child_notify);

    // Delete parent folder
    mail_api_labels::mocks::mock_delete_label(parent_folder.remote_id.clone().unwrap(), 200)
        .expect(1)
        .mount(ctx.mock_server())
        .await;
    let action = Delete::new(parent_folder.local_id.unwrap());
    user_ctx.action_queue().queue_action(action).await.unwrap();
    user_ctx.execute_single_action().await.unwrap();
    let folders = Label::find_by_kind(LabelType::Folder, &tether)
        .await
        .unwrap();
    assert_eq!(folders.len(), 0);
}

#[tokio::test]
async fn test_fail_create_label() {
    let ctx = MailTestContext::new().await;
    let user_ctx = ctx.uninitialized_mail_user_context().await;
    let tether = user_ctx.user_stash().connection();

    ctx.setup_user(Params::default_basic()).await;
    ctx.initialize_uninitialized_ctx(&user_ctx).await;

    // Attempt create label
    let name = "My Awesome Label";
    let color = WellKnownLabelColor::Pickle;
    mail_api_labels::mocks::mock_create_label(name.into(), color.hex_code().to_owned(), Err(400))
        .expect(1)
        .mount(ctx.mock_server())
        .await;
    let action = Create::new_custom_label(name.into(), color);
    user_ctx.action_queue().queue_action(action).await.unwrap();
    user_ctx.execute_single_action().await.unwrap_err();

    // Check label
    let labels = Label::find_by_kind(LabelType::Label, &tether)
        .await
        .unwrap();
    assert_eq!(labels.len(), 1);
}

#[tokio::test]
async fn test_fail_delete_label() {
    let ctx = MailTestContext::new().await;
    let user_ctx = ctx.uninitialized_mail_user_context().await;
    let tether = user_ctx.user_stash().connection();

    ctx.setup_user(Params::default_basic()).await;
    ctx.initialize_uninitialized_ctx(&user_ctx).await;

    // Create label
    let name = "My Awesome Label";
    let color = WellKnownLabelColor::Forest;
    mail_api_labels::mocks::mock_create_label(
        name.into(),
        color.hex_code().to_owned(),
        Ok("label-1".into()),
    )
    .expect(1)
    .mount(ctx.mock_server())
    .await;
    let action = Create::new_custom_label(name.into(), color);
    user_ctx.action_queue().queue_action(action).await.unwrap();
    user_ctx.execute_single_action().await.unwrap();

    // Check label
    let label = Label::find_by_name_kind(name.to_owned(), LabelType::Label, &tether)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(label.color, LabelColor::from(color));
    assert!(label.notify);

    // Attempt delete label
    mail_api_labels::mocks::mock_delete_label(label.remote_id.unwrap(), 400)
        .expect(1)
        .mount(ctx.mock_server())
        .await;
    let action = Delete::new(label.local_id.unwrap());
    user_ctx.action_queue().queue_action(action).await.unwrap();
    user_ctx.execute_single_action().await.unwrap_err();
    let labels = Label::find_by_kind(LabelType::Label, &tether)
        .await
        .unwrap();
    assert_eq!(labels.len(), 2);
}

#[tokio::test]
async fn test_fail_put_label() {
    let ctx = MailTestContext::new().await;
    let user_ctx = ctx.uninitialized_mail_user_context().await;
    let tether = user_ctx.user_stash().connection();

    ctx.setup_user(Params::default_basic()).await;
    ctx.initialize_uninitialized_ctx(&user_ctx).await;

    // Create label
    let name = "My Awesome Label";
    let color = WellKnownLabelColor::Cerise;
    mail_api_labels::mocks::mock_create_label(
        name.into(),
        color.hex_code().to_owned(),
        Ok("label-1".into()),
    )
    .expect(1)
    .mount(ctx.mock_server())
    .await;
    let action = Create::new_custom_label(name.into(), color);
    user_ctx.action_queue().queue_action(action).await.unwrap();
    user_ctx.execute_single_action().await.unwrap();

    // Check label
    let label = Label::find_by_name_kind(name.to_owned(), LabelType::Label, &tether)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(label.color, LabelColor::from(color));
    assert!(label.notify);

    // Attempt Edit label
    let new_name = "My Better Label";
    let new_color = WellKnownLabelColor::Slateblue;
    mail_api_labels::mocks::mock_put_label(
        "label-1".into(),
        new_name.to_owned(),
        new_color.hex_code().to_owned(),
        Err(400),
    )
    .expect(1)
    .mount(ctx.mock_server())
    .await;
    let action = Update::new_custom_label(label.local_id.unwrap(), new_name.into(), new_color);
    user_ctx.action_queue().queue_action(action).await.unwrap();
    user_ctx.execute_single_action().await.unwrap_err();

    // Check label
    let label = Label::find_by_name_kind(name.to_owned(), LabelType::Label, &tether)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(label.color, LabelColor::from(color));
    assert!(label.notify);
}

#[tokio::test]
async fn test_reparent_custom_folder() {
    let ctx = MailTestContext::new().await;
    let user_ctx = ctx.uninitialized_mail_user_context().await;
    let tether = user_ctx.user_stash().connection();

    ctx.setup_user(Params::default_basic()).await;
    ctx.initialize_uninitialized_ctx(&user_ctx).await;

    // Create parent folder A
    let parent_a_name = "Parent Folder A";
    let parent_a_color = WellKnownLabelColor::Enzian;
    mail_api_labels::mocks::mock_create_folder(
        None,
        parent_a_name.into(),
        parent_a_color.hex_code().to_owned(),
        true,
        Ok("folder-1".into()),
    )
    .expect(1)
    .mount(ctx.mock_server())
    .await;
    let action = Create::new_custom_folder(None, parent_a_name.into(), parent_a_color, true);
    user_ctx.action_queue().queue_action(action).await.unwrap();
    user_ctx.execute_single_action().await.unwrap();

    let parent_a = Label::find_by_name_kind(parent_a_name.to_owned(), LabelType::Folder, &tether)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(parent_a.local_parent_id, None);

    // Create child folder under parent A
    let child_name = "Child Folder";
    let child_color = WellKnownLabelColor::Reef;
    let child_notify = false;
    mail_api_labels::mocks::mock_create_folder(
        Some(parent_a.remote_id.clone().unwrap()),
        child_name.into(),
        child_color.hex_code().to_owned(),
        child_notify,
        Ok("folder-2".into()),
    )
    .expect(1)
    .mount(ctx.mock_server())
    .await;
    let action = Create::new_custom_folder(
        Some(parent_a.local_id.unwrap()),
        child_name.into(),
        child_color,
        child_notify,
    );
    user_ctx.action_queue().queue_action(action).await.unwrap();
    user_ctx.execute_single_action().await.unwrap();

    let child = Label::find_by_name_kind(child_name.to_owned(), LabelType::Folder, &tether)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(child.local_parent_id, Some(parent_a.local_id.unwrap()));
    assert_eq!(
        child.remote_parent_id,
        Some(parent_a.remote_id.clone().unwrap())
    );

    // Create parent folder B
    let parent_b_name = "Parent Folder B";
    let parent_b_color = WellKnownLabelColor::Pink;
    mail_api_labels::mocks::mock_create_folder(
        None,
        parent_b_name.into(),
        parent_b_color.hex_code().to_owned(),
        true,
        Ok("folder-3".into()),
    )
    .expect(1)
    .mount(ctx.mock_server())
    .await;
    let action = Create::new_custom_folder(None, parent_b_name.into(), parent_b_color, true);
    user_ctx.action_queue().queue_action(action).await.unwrap();
    user_ctx.execute_single_action().await.unwrap();

    let parent_b = Label::find_by_name_kind(parent_b_name.to_owned(), LabelType::Folder, &tether)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(parent_b.local_parent_id, None);

    // Reparent child from A to B
    mail_api_labels::mocks::mock_put_folder(
        child.remote_id.clone().unwrap(),
        Some(parent_b.remote_id.clone().unwrap()),
        child_name.into(),
        child_color.hex_code().to_owned(),
        child_notify,
        Ok(()),
    )
    .expect(1)
    .mount(ctx.mock_server())
    .await;
    let action = Update::new_custom_folder(
        child.local_id.unwrap(),
        Some(parent_b.local_id.unwrap()),
        child_name.into(),
        child_color,
        child_notify,
    );
    user_ctx.action_queue().queue_action(action).await.unwrap();
    user_ctx.execute_single_action().await.unwrap();

    // Verify child is now under parent B
    let child = Label::find_by_name_kind(child_name.to_owned(), LabelType::Folder, &tether)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(child.local_parent_id, Some(parent_b.local_id.unwrap()));
    assert_eq!(
        child.remote_parent_id,
        Some(parent_b.remote_id.clone().unwrap())
    );
}

async fn setup_grandchildren_hierarchy(
    ctx: &MailTestContext,
    user_ctx: &Arc<MailUserContext>,
    tether: &Tether,
) -> (Label, Label, Label) {
    // Create root folder
    let root_color = WellKnownLabelColor::Enzian;
    mail_api_labels::mocks::mock_create_folder(
        None,
        "Root".into(),
        root_color.hex_code().to_owned(),
        true,
        Ok("folder-root".into()),
    )
    .expect(1)
    .mount(ctx.mock_server())
    .await;
    let action = Create::new_custom_folder(None, "Root".into(), root_color, true);
    user_ctx.action_queue().queue_action(action).await.unwrap();
    user_ctx.execute_single_action().await.unwrap();
    let root = Label::find_by_name_kind("Root".to_owned(), LabelType::Folder, tether)
        .await
        .unwrap()
        .unwrap();

    // Create child folder under root
    let child_color = WellKnownLabelColor::Reef;
    mail_api_labels::mocks::mock_create_folder(
        Some(root.remote_id.clone().unwrap()),
        "Child".into(),
        child_color.hex_code().to_owned(),
        true,
        Ok("folder-child".into()),
    )
    .expect(1)
    .mount(ctx.mock_server())
    .await;
    let action = Create::new_custom_folder(
        Some(root.local_id.unwrap()),
        "Child".into(),
        child_color,
        true,
    );
    user_ctx.action_queue().queue_action(action).await.unwrap();
    user_ctx.execute_single_action().await.unwrap();
    let child = Label::find_by_name_kind("Child".to_owned(), LabelType::Folder, tether)
        .await
        .unwrap()
        .unwrap();

    // Create grandchild folder under child
    let grandchild_color = WellKnownLabelColor::Forest;
    mail_api_labels::mocks::mock_create_folder(
        Some(child.remote_id.clone().unwrap()),
        "Grandchild".into(),
        grandchild_color.hex_code().to_owned(),
        true,
        Ok("folder-grandchild".into()),
    )
    .expect(1)
    .mount(ctx.mock_server())
    .await;
    let action = Create::new_custom_folder(
        Some(child.local_id.unwrap()),
        "Grandchild".into(),
        grandchild_color,
        true,
    );
    user_ctx.action_queue().queue_action(action).await.unwrap();
    user_ctx.execute_single_action().await.unwrap();
    let grandchild = Label::find_by_name_kind("Grandchild".to_owned(), LabelType::Folder, tether)
        .await
        .unwrap()
        .unwrap();
    (root, child, grandchild)
}

#[tokio::test]
async fn test_find_descendants_returns_all_nested_levels() {
    let ctx = MailTestContext::new().await;
    let user_ctx = ctx.uninitialized_mail_user_context().await;
    let tether = user_ctx.user_stash().connection();

    ctx.setup_user(Params::default_basic()).await;
    ctx.initialize_uninitialized_ctx(&user_ctx).await;

    let (root, child, grandchild) = setup_grandchildren_hierarchy(&ctx, &user_ctx, &tether).await;

    // find_descendants from root must include both child and grandchild
    let mut descendants = Label::find_descendants(&tether, root.local_id.unwrap())
        .await
        .unwrap();
    descendants.sort();

    let mut expected = vec![child.local_id.unwrap(), grandchild.local_id.unwrap()];
    expected.sort();

    assert_eq!(
        descendants, expected,
        "find_descendants should return all levels of nesting, not just direct children"
    );
}

#[tokio::test]
async fn rebase_delete() {
    let ctx = MailTestContext::new().await;
    let user_ctx = ctx.uninitialized_mail_user_context().await;
    let tether = user_ctx.user_stash().connection();

    ctx.setup_user(Params::default_basic()).await;
    ctx.initialize_uninitialized_ctx(&user_ctx).await;

    let (root, _child, _grandchild) = setup_grandchildren_hierarchy(&ctx, &user_ctx, &tether).await;

    let action = Delete::new(root.local_id.unwrap());
    user_ctx.action_queue().queue_action(action).await.unwrap();

    assert!(
        Label::find_by_name_kind("Root".to_owned(), LabelType::Folder, &tether)
            .await
            .unwrap()
            .is_none(),
        "root folder should be deleted",
    );

    user_ctx
        .action_queue()
        .rebase(
            ActionGroup::default(),
            &RebaseChangeSet::from(root.local_id.unwrap().to_string()),
        )
        .await
        .unwrap();

    assert!(
        Label::find_by_name_kind("Root".to_owned(), LabelType::Folder, &tether)
            .await
            .unwrap()
            .is_none(),
        "root folder should be deleted",
    );

    mail_api_labels::mocks::mock_delete_label(root.remote_id.unwrap(), 200)
        .expect(1)
        .mount(ctx.mock_server())
        .await;
    user_ctx.execute_single_action().await.unwrap();
}

#[tokio::test]
async fn rebase_children_delete() {
    let ctx = MailTestContext::new().await;
    let user_ctx = ctx.uninitialized_mail_user_context().await;
    let tether = user_ctx.user_stash().connection();

    ctx.setup_user(Params::default_basic()).await;
    ctx.initialize_uninitialized_ctx(&user_ctx).await;

    let (root, _child, _grandchild) = setup_grandchildren_hierarchy(&ctx, &user_ctx, &tether).await;

    let action = Delete::new(root.local_id.unwrap());
    user_ctx.action_queue().queue_action(action).await.unwrap();
    assert!(
        Label::find_by_name_kind("Root".to_owned(), LabelType::Folder, &tether)
            .await
            .unwrap()
            .is_none(),
        "Root folder should be deleted",
    );
    assert!(
        Label::find_by_name_kind("Child".to_owned(), LabelType::Folder, &tether)
            .await
            .unwrap()
            .is_none(),
        "Child folder should be deleted",
    );

    user_ctx
        .action_queue()
        .rebase(
            ActionGroup::default(),
            &RebaseChangeSet::from(root.local_id.unwrap()),
        )
        .await
        .unwrap();

    assert!(
        Label::find_by_name_kind("Root".to_owned(), LabelType::Folder, &tether)
            .await
            .unwrap()
            .is_none(),
        "Root folder should be deleted",
    );
    assert!(
        Label::find_by_name_kind("Child".to_owned(), LabelType::Folder, &tether)
            .await
            .unwrap()
            .is_none(),
        "Child folder should be deleted",
    );

    mail_api_labels::mocks::mock_delete_label(root.remote_id.unwrap(), 200)
        .expect(1)
        .mount(ctx.mock_server())
        .await;
    user_ctx.execute_single_action().await.unwrap().unwrap();
}

#[tokio::test]
async fn double_delete_action_revert() {
    let ctx = MailTestContext::new().await;
    let user_ctx = ctx.uninitialized_mail_user_context().await;
    let tether = user_ctx.user_stash().connection();

    ctx.setup_user(Params::default_basic()).await;
    ctx.initialize_uninitialized_ctx(&user_ctx).await;

    let (root, child, _grandchild) = setup_grandchildren_hierarchy(&ctx, &user_ctx, &tether).await;

    // Enqueue a delete action on the child
    let action = Delete::new(child.local_id.unwrap());
    user_ctx.action_queue().queue_action(action).await.unwrap();
    assert!(
        Label::find_by_name_kind("Child".to_owned(), LabelType::Folder, &tether)
            .await
            .unwrap()
            .is_none(),
        "Child folder should be deleted",
    );

    // Enqueue a delete action on the root
    let action = Delete::new(root.local_id.unwrap());
    user_ctx.action_queue().queue_action(action).await.unwrap();
    assert!(
        Label::find_by_name_kind("Root".to_owned(), LabelType::Folder, &tether)
            .await
            .unwrap()
            .is_none(),
        "Root folder should be deleted",
    );
    assert!(
        Label::find_by_name_kind("Child".to_owned(), LabelType::Folder, &tether)
            .await
            .unwrap()
            .is_none(),
        "Child folder should be deleted",
    );

    // Child delete fails
    mail_api_labels::mocks::mock_delete_label(child.remote_id.unwrap(), 400)
        .expect(1)
        .mount(ctx.mock_server())
        .await;
    user_ctx.execute_single_action().await.unwrap_err();
    assert!(
        Label::find_by_name_kind("Root".to_owned(), LabelType::Folder, &tether)
            .await
            .unwrap()
            .is_none(),
        "Root folder should be deleted",
    );
    assert!(
        Label::find_by_name_kind("Child".to_owned(), LabelType::Folder, &tether)
            .await
            .unwrap()
            .is_none(),
        "Child folder should be deleted still because there is still a pending action",
    );

    mail_api_labels::mocks::mock_delete_label(root.remote_id.unwrap(), 200)
        .expect(1)
        .mount(ctx.mock_server())
        .await;
    user_ctx.execute_single_action().await.unwrap();
    assert!(
        Label::find_by_name_kind("Root".to_owned(), LabelType::Folder, &tether)
            .await
            .unwrap()
            .is_none(),
        "Root folder should be deleted",
    );
    assert!(
        Label::find_by_name_kind("Child".to_owned(), LabelType::Folder, &tether)
            .await
            .unwrap()
            .is_none(),
        "Child folder should be deleted",
    );
}

#[tokio::test]
async fn delete_rebase_root_renamed_remotely() {
    let ctx = MailTestContext::new().await;
    let user_ctx = ctx.uninitialized_mail_user_context().await;
    let mut tether = user_ctx.user_stash().connection();

    ctx.setup_user(Params::default_basic()).await;
    ctx.initialize_uninitialized_ctx(&user_ctx).await;

    let (mut root, _child, _grandchild) =
        setup_grandchildren_hierarchy(&ctx, &user_ctx, &tether).await;

    let action = Delete::new(root.local_id.unwrap());
    user_ctx.action_queue().queue_action(action).await.unwrap();
    assert!(
        Label::find_by_name_kind("Root".to_owned(), LabelType::Folder, &tether)
            .await
            .unwrap()
            .is_none(),
        "Root folder should be deleted",
    );
    assert!(
        Label::find_by_name_kind("Child".to_owned(), LabelType::Folder, &tether)
            .await
            .unwrap()
            .is_none(),
        "Child folder should be deleted",
    );
    let root_label_id = root.local_id.unwrap();

    let mut rebase_change_set = RebaseChangeSet::default();
    // Rename the label remotely
    tether
        .write_tx(async |tx| {
            root.name = "Renamed".to_owned();
            Label::handle_event(
                tx,
                &root.remote_id.clone().unwrap(),
                mail_core_common::event_loop::events::Action::Update,
                Some(&mut root),
                &mut rebase_change_set,
            )
            .await
        })
        .await
        .unwrap();
    let root = Label::load(root_label_id, &tether).await.unwrap().unwrap();
    assert_eq!(&root.name, "Renamed");
    // Label should still be deleted
    assert_eq!(Label::get_deleted(&tether, root_label_id).await.unwrap(), 1);

    // Run the rebase
    user_ctx
        .action_queue()
        .rebase(ActionGroup::default(), &rebase_change_set)
        .await
        .unwrap();
    // Label should still be deleted
    assert_eq!(Label::get_deleted(&tether, root_label_id).await.unwrap(), 1);
}

#[tokio::test]
async fn delete_rebase_child_deparent_remotely() {
    let ctx = MailTestContext::new().await;
    let user_ctx = ctx.uninitialized_mail_user_context().await;
    let mut tether = user_ctx.user_stash().connection();

    ctx.setup_user(Params::default_basic()).await;
    ctx.initialize_uninitialized_ctx(&user_ctx).await;

    let (root, mut child, _grandchild) =
        setup_grandchildren_hierarchy(&ctx, &user_ctx, &tether).await;

    let action = Delete::new(root.local_id.unwrap());
    user_ctx.action_queue().queue_action(action).await.unwrap();
    assert!(
        Label::find_by_name_kind("Root".to_owned(), LabelType::Folder, &tether)
            .await
            .unwrap()
            .is_none(),
        "Root folder should be deleted",
    );
    assert!(
        Label::find_by_name_kind("Child".to_owned(), LabelType::Folder, &tether)
            .await
            .unwrap()
            .is_none(),
        "Child folder should be deleted",
    );
    let child_label_id = child.local_id.unwrap();
    let mut rebase_change_set = RebaseChangeSet::default();
    // Deparent the label remotely
    tether
        .write_tx(async |tx| {
            child.local_parent_id = None;
            child.remote_parent_id = None;
            Label::handle_event(
                tx,
                &child.remote_id.clone().unwrap(),
                mail_core_common::event_loop::events::Action::Update,
                Some(&mut child),
                &mut rebase_change_set,
            )
            .await
        })
        .await
        .unwrap();
    // Label should still be deleted
    assert_eq!(
        Label::get_deleted(&tether, child_label_id).await.unwrap(),
        1
    );

    // Run the rebase
    user_ctx
        .action_queue()
        .rebase(ActionGroup::default(), &rebase_change_set)
        .await
        .unwrap();
    // Label should now be restored since its parent was switched
    assert_eq!(
        Label::get_deleted(&tether, child_label_id).await.unwrap(),
        0
    );

    mail_api_labels::mocks::mock_delete_label(dbg!(root.remote_id.unwrap()), 200)
        .expect(1)
        .mount(ctx.mock_server())
        .await;
    user_ctx.execute_single_action().await.unwrap().unwrap();
}

#[tokio::test]
async fn update_rebase_revert_child_rename_remotely() {
    let ctx = MailTestContext::new().await;
    let user_ctx = ctx.uninitialized_mail_user_context().await;
    let mut tether = user_ctx.user_stash().connection();

    ctx.setup_user(Params::default_basic()).await;
    ctx.initialize_uninitialized_ctx(&user_ctx).await;

    let (_root, child, _grandchild) = setup_grandchildren_hierarchy(&ctx, &user_ctx, &tether).await;
    assert_eq!(
        &child.color.to_string(),
        WellKnownLabelColor::Reef.hex_code()
    );

    // Create an update action that changes the name and color
    let action = Update::new_custom_folder(
        child.local_id.unwrap(),
        Some(child.local_parent_id.unwrap()),
        "New child name".to_owned(),
        WellKnownLabelColor::Olive,
        child.notify,
    );
    user_ctx.action_queue().queue_action(action).await.unwrap();

    let child_label_id = child.local_id.unwrap();
    let mut child = Label::load(child_label_id, &tether).await.unwrap().unwrap();
    assert_eq!(&child.name, "New child name");
    assert_eq!(
        &child.color.to_string(),
        WellKnownLabelColor::Olive.hex_code()
    );

    let mut rebase_change_set = RebaseChangeSet::default();
    // Rename the child remotely while changing the color back
    tether
        .write_tx(async |tx| {
            child.name = "New remote child name".to_owned();
            child.color = WellKnownLabelColor::Reef.into();
            Label::handle_event(
                tx,
                &child.remote_id.clone().unwrap(),
                mail_core_common::event_loop::events::Action::Update,
                Some(&mut child),
                &mut rebase_change_set,
            )
            .await
        })
        .await
        .unwrap();
    let child = Label::load(child_label_id, &tether).await.unwrap().unwrap();
    assert_eq!(&child.name, "New remote child name");
    assert_eq!(
        &child.color.to_string(),
        WellKnownLabelColor::Reef.hex_code()
    );

    // Run the rebase
    user_ctx
        .action_queue()
        .rebase(ActionGroup::default(), &rebase_change_set)
        .await
        .unwrap();
    // Label name should now be rebased and changed back
    let child = Label::load(child_label_id, &tether).await.unwrap().unwrap();
    assert_eq!(&child.name, "New child name");
    assert_eq!(
        &child.color.to_string(),
        WellKnownLabelColor::Olive.hex_code()
    );

    // Now lets fail the action to apply_remote so we can test revert
    mail_api_labels::mocks::mock_put_folder(
        child.remote_id.clone().unwrap(),
        Some(child.remote_parent_id.clone().unwrap()),
        "New child name".to_owned(),
        child.color.to_string(),
        child.notify,
        Err(400),
    )
    .expect(1)
    .mount(ctx.mock_server())
    .await;
    user_ctx.execute_single_action().await.unwrap_err();

    // Label name should now be reverted to the remote name and color to the remote color
    let child = Label::load(child_label_id, &tether).await.unwrap().unwrap();
    assert_eq!(&child.name, "New remote child name");
    assert_eq!(
        &child.color.to_string(),
        WellKnownLabelColor::Reef.hex_code()
    );
}

#[tokio::test]
async fn delete_rebase_create_child_remotely() {
    let ctx = MailTestContext::new().await;
    let user_ctx = ctx.uninitialized_mail_user_context().await;
    let mut tether = user_ctx.user_stash().connection();

    ctx.setup_user(Params::default_basic()).await;
    ctx.initialize_uninitialized_ctx(&user_ctx).await;

    // Create folder 1
    mail_api_labels::mocks::mock_create_folder(
        None,
        "My Folder 1".into(),
        WellKnownLabelColor::Enzian.hex_code().to_owned(),
        true,
        Ok("folder-1".into()),
    )
    .expect(1)
    .mount(ctx.mock_server())
    .await;
    let action = Create::new_custom_folder(
        None,
        "My Folder 1".into(),
        WellKnownLabelColor::Enzian,
        true,
    );
    user_ctx.action_queue().queue_action(action).await.unwrap();
    user_ctx.execute_single_action().await.unwrap();
    let folder1 = Label::find_by_name_kind("My Folder 1".to_owned(), LabelType::Folder, &tether)
        .await
        .unwrap()
        .unwrap();

    // Delete folder 1 locally
    let action = Delete::new(folder1.local_id.unwrap());
    user_ctx.action_queue().queue_action(action).await.unwrap();

    let mut rebase_change_set = RebaseChangeSet::default();
    // Remotely folder 2 becomes the child of folder 1
    tether
        .write_tx(async |tx| {
            let mut folder2 = Label {
                local_id: None,
                remote_id: Some(LabelId::new("folder-2".into())),
                local_parent_id: Some(folder1.local_id.unwrap()),
                remote_parent_id: Some(folder1.remote_id.unwrap()),
                color: WellKnownLabelColor::Carrot.into(),
                display: true,
                expanded: true,
                label_type: LabelType::Folder,
                name: "My Folder 2".into(),
                notify: true,
                display_order: 2,
                path: None,
                sticky: false,
                last_unseen_message: None,
            };
            Label::handle_event(
                tx,
                &folder2.remote_id.clone().unwrap(),
                mail_core_common::event_loop::events::Action::Create,
                Some(&mut folder2),
                &mut rebase_change_set,
            )
            .await
        })
        .await
        .unwrap();

    let folder2 = Label::find_by_name_kind("My Folder 2".to_owned(), LabelType::Folder, &tether)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        Label::get_deleted(&tether, folder1.local_id.unwrap())
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        Label::get_deleted(&tether, folder2.local_id.unwrap())
            .await
            .unwrap(),
        0
    );

    // Run the rebase
    user_ctx
        .action_queue()
        .rebase(ActionGroup::default(), &rebase_change_set)
        .await
        .unwrap();
    assert_eq!(
        Label::get_deleted(&tether, folder1.local_id.unwrap())
            .await
            .unwrap(),
        1
    );
    // After the rebase folder 2 should be marked for deletion too
    assert_eq!(
        Label::get_deleted(&tether, folder2.local_id.unwrap())
            .await
            .unwrap(),
        1
    );
}

#[tokio::test]
async fn delete_rebase_reparent_remotely() {
    let ctx = MailTestContext::new().await;
    let user_ctx = ctx.uninitialized_mail_user_context().await;
    let mut tether = user_ctx.user_stash().connection();

    ctx.setup_user(Params::default_basic()).await;
    ctx.initialize_uninitialized_ctx(&user_ctx).await;

    // Create folder 1
    mail_api_labels::mocks::mock_create_folder(
        None,
        "My Folder 1".into(),
        WellKnownLabelColor::Enzian.hex_code().to_owned(),
        true,
        Ok("folder-1".into()),
    )
    .expect(1)
    .mount(ctx.mock_server())
    .await;
    let action = Create::new_custom_folder(
        None,
        "My Folder 1".into(),
        WellKnownLabelColor::Enzian,
        true,
    );
    user_ctx.action_queue().queue_action(action).await.unwrap();
    user_ctx.execute_single_action().await.unwrap();
    let folder1 = Label::find_by_name_kind("My Folder 1".to_owned(), LabelType::Folder, &tether)
        .await
        .unwrap()
        .unwrap();

    // Create folder 2
    mail_api_labels::mocks::mock_create_folder(
        None,
        "My Folder 2".into(),
        WellKnownLabelColor::Reef.hex_code().to_owned(),
        true,
        Ok("folder-2".into()),
    )
    .expect(1)
    .mount(ctx.mock_server())
    .await;
    let action =
        Create::new_custom_folder(None, "My Folder 2".into(), WellKnownLabelColor::Reef, true);
    user_ctx.action_queue().queue_action(action).await.unwrap();
    user_ctx.execute_single_action().await.unwrap();
    let mut folder2 = Label::find_by_name_kind("My Folder 2".into(), LabelType::Folder, &tether)
        .await
        .unwrap()
        .unwrap();

    // Delete folder 1 locally
    let action = Delete::new(folder1.local_id.unwrap());
    user_ctx.action_queue().queue_action(action).await.unwrap();

    let mut rebase_change_set = RebaseChangeSet::default();
    // Remotely folder 2 becomes the child of folder 1
    tether
        .write_tx(async |tx| {
            folder2.local_parent_id = Some(folder1.local_id.unwrap());
            folder2.remote_parent_id = Some(folder1.remote_id.unwrap());
            Label::handle_event(
                tx,
                &folder2.remote_id.clone().unwrap(),
                mail_core_common::event_loop::events::Action::Update,
                Some(&mut folder2),
                &mut rebase_change_set,
            )
            .await
        })
        .await
        .unwrap();

    assert_eq!(
        Label::get_deleted(&tether, folder1.local_id.unwrap())
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        Label::get_deleted(&tether, folder2.local_id.unwrap())
            .await
            .unwrap(),
        0
    );

    // Run the rebase
    user_ctx
        .action_queue()
        .rebase(ActionGroup::default(), &rebase_change_set)
        .await
        .unwrap();
    assert_eq!(
        Label::get_deleted(&tether, folder1.local_id.unwrap())
            .await
            .unwrap(),
        1
    );
    // After the rebase folder 2 should be marked for deletion too
    assert_eq!(
        Label::get_deleted(&tether, folder2.local_id.unwrap())
            .await
            .unwrap(),
        1
    );
}
