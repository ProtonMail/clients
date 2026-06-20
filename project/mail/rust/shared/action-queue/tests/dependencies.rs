mod common;
use mail_action_queue::action::{
    Action, ActionDependencyKeys, DefaultVersionConverter, Handler, MetadataBuilder, NoopError,
    Priority, Type,
};
use mail_action_queue::queue::QueuedActionState;
use mail_action_queue::tests::common::TestDb;
use serde::{Deserialize, Serialize};

use common::new_queue_typed;

#[derive(Clone, Serialize, Deserialize)]
struct TestAction {
    required_dependencies: Vec<String>,
    optional_dependencies: Vec<String>,
    records: Vec<String>,
}

impl Action<TestDb> for TestAction {
    const TYPE: Type = Type("test_action");
    const VERSION: u32 = 1;

    type VersionConverter = DefaultVersionConverter<Self>;
    type Handler = TestActionHandler;
    type RemoteOutput = ();
    type LocalOutput = ();
    type Error = NoopError;

    fn dependency_keys(&self) -> ActionDependencyKeys {
        ActionDependencyKeys {
            required: self.required_dependencies.iter().map(Into::into).collect(),
            optional: self.optional_dependencies.iter().map(Into::into).collect(),
            record: self.records.iter().map(Into::into).collect(),
        }
    }
}

struct TestActionHandler;

impl Handler<TestDb> for TestActionHandler {
    type Action = TestAction;

    async fn apply_local(
        &self,
        _id: mail_action_queue::action::ActionId,
        _action: &mut Self::Action,
        _tx: &mail_stash::stash::WriteTx<'_, TestDb>,
    ) -> Result<(), NoopError> {
        Ok(())
    }

    async fn revert_local(
        &self,
        _this_id: mail_action_queue::action::ActionId,
        _action: &mut Self::Action,
        _tx: &mail_stash::stash::WriteTx<'_, TestDb>,
    ) -> Result<(), NoopError> {
        Ok(())
    }

    async fn apply_remote(
        &self,
        _this_id: mail_action_queue::action::ActionId,
        _action: &mut Self::Action,
    ) -> Result<(), NoopError> {
        Ok(())
    }

    async fn rebase_local(
        &self,
        _id: mail_action_queue::action::ActionId,
        _action: &mut Self::Action,
        _change_set: &mail_action_queue::rebase::RebaseChangeSet,
        _tx: &mail_stash::stash::WriteTx<'_, TestDb>,
    ) -> Result<(), NoopError> {
        Ok(())
    }
}

#[tokio::test]
async fn actions_post_apply_local() {
    let queue = new_queue_typed::<TestAction>(TestActionHandler).await;

    let id0 = queue
        .queue_action_with_metadata(
            TestAction {
                required_dependencies: vec![],
                optional_dependencies: vec![],
                records: vec!["dep-1".to_owned()],
            },
            MetadataBuilder::new()
                .with_priority_override(Priority::Lowest)
                .build(),
        )
        .await
        .unwrap()
        .id;

    let id1 = queue
        .queue_action(TestAction {
            required_dependencies: vec!["dep-1".to_owned()],
            optional_dependencies: vec![],
            records: vec![],
        })
        .await
        .unwrap()
        .id;

    let executor = queue.new_executor();

    let action = executor.execute_one().await.unwrap().unwrap();
    let QueuedActionState::Executed(id) = action else {
        panic!("action should have been executed");
    };
    assert_eq!(id0, id);

    let action = executor.execute_one().await.unwrap().unwrap();
    let QueuedActionState::Executed(id) = action else {
        panic!("action should have been executed");
    };
    assert_eq!(id1, id);
}
