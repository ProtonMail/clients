#![allow(clippy::module_inception)]
#![allow(clippy::struct_excessive_bools)]

use crate::label_type::{ALL_LABEL_TYPES, LabelColor, LabelType, MAIL_LABEL_TYPES};
use crate::local_ids::LocalLabelId;
use itertools::Itertools;
use mail_action_queue::rebase::RebaseChangeSet;
use mail_core_api::service::ApiServiceError;
use mail_core_api::services::proton::{
    EventId, Label as ApiLabel, LabelId, PatchLabelRequest, ProtonCore,
};
use mail_shared_types::{Action, InitializationKey, ModelIdExtension};
use mail_stash::exports::{Connection, Transaction};
use mail_stash::macros::Model;
use mail_stash::orm::{Model, ModelHooks};
use mail_stash::stash::{Stash, StashError, StashResult, Tether, WatcherHandle, WriteTx};
use mail_stash::utils::{MapToSql as _, placeholders};
use mail_stash::{UserDb, params};
use sqlite_watcher::watcher::TableObserver;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;
use topological_sort::TopologicalSort;
use tracing::log::warn;
use tracing::{error, instrument};

#[derive(Debug, Error)]
pub enum LabelError {
    #[error("API error: {0}")]
    API(#[from] ApiServiceError),
    #[error("Stash error: {0}")]
    Stash(#[from] StashError),
    #[error("Could not resolve remote label: {0}")]
    CouldNotResolveRemoteLabel(LocalLabelId),
    #[error("Could not resolve local label: {0}")]
    CouldNotResolveLocalLabel(LabelId),
    #[error("Label does not have neither remote nor local id")]
    LabelWithoutIds,
}

#[derive(Clone, Debug, Eq, Model, PartialEq)]
#[ModelHooks]
#[TableName("labels")]
#[Database(UserDb)]
pub struct Label {
    #[IdField(autoincrement)]
    pub local_id: Option<LocalLabelId>,

    #[DbField]
    pub remote_id: Option<LabelId>,

    #[DbField]
    pub local_parent_id: Option<LocalLabelId>,

    #[DbField]
    pub remote_parent_id: Option<LabelId>,

    #[DbField]
    pub color: LabelColor,

    #[DbField]
    pub display: bool,

    #[DbField]
    pub expanded: bool,

    #[DbField]
    pub label_type: LabelType,

    #[DbField]
    pub name: String,

    #[DbField]
    pub notify: bool,

    #[DbField]
    pub display_order: u32,

    #[DbField]
    pub path: Option<String>,

    #[DbField]
    pub sticky: bool,

    #[DbField]
    /// Whenever this field is Some(EventID)
    /// clients need to show unseen badge for
    /// given category label
    pub last_unseen_message: Option<EventId>,
}

impl ModelIdExtension for Label {
    type RemoteId = LabelId;

    fn remote_id(&self) -> Option<&Self::RemoteId> {
        self.remote_id.as_ref()
    }
}

impl Label {
    pub const INIT_KEY: InitializationKey = InitializationKey::new("labels");

    #[instrument(skip_all)]
    pub async fn all_labels<API>(api: &API) -> Result<Vec<Label>, LabelError>
    where
        API: ProtonCore,
    {
        Self::fetch_labels(api, &ALL_LABEL_TYPES).await
    }

    #[instrument(skip_all)]
    pub async fn fetch_mail_labels<API>(api: &API) -> Result<Vec<Label>, LabelError>
    where
        API: ProtonCore,
    {
        Self::fetch_labels(api, &MAIL_LABEL_TYPES).await
    }

    #[allow(clippy::result_large_err)]
    #[instrument(skip_all)]
    pub async fn fetch_labels<API>(
        api: &API,
        label_types: &[LabelType],
    ) -> Result<Vec<Label>, LabelError>
    where
        API: ProtonCore,
    {
        let labels = futures::future::join_all(
            label_types
                .iter()
                .map(|category| api.get_labels((*category).into())),
        )
        .await;

        let labels = labels
            .into_iter()
            .map(|res| {
                res.inspect_err(|err| error!("Failed to fetch labels: {err:?}"))
                    .map_err(LabelError::from)
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flat_map(|res| res.labels);

        Ok(Self::topo_sort(labels))
    }

    pub fn topo_sort<L: Into<Label>>(labels: impl IntoIterator<Item = L>) -> Vec<Label> {
        let mut ids = TopologicalSort::<LabelId>::new();
        let mut objs = BTreeMap::new();

        for label in labels {
            let label = label.into();
            let rid = label.remote_id.clone().unwrap();
            if let Some(parent_id) = &label.remote_parent_id {
                ids.add_dependency(parent_id.clone(), rid.clone());
            } else {
                ids.insert(rid.clone());
            }

            objs.entry(rid).or_insert(label);
        }

        let mut labels = Vec::new();

        while let Some(id) = ids.pop() {
            if let Some(obj) = objs.remove(&id) {
                labels.push(obj);
            }
        }

        labels
    }

    #[instrument(skip_all)]
    pub async fn get_labels_by_ids<API>(
        api: &API,
        ids: Vec<LabelId>,
    ) -> Result<Vec<Label>, LabelError>
    where
        API: ProtonCore,
    {
        Ok(api
            .get_labels_by_ids(ids)
            .await?
            .labels
            .into_iter()
            .map_into::<Self>()
            .collect())
    }

    #[instrument(skip_all)]
    pub async fn store_labels_async(
        tx: &WriteTx<'_>,
        labels: Vec<Label>,
    ) -> StashResult<Vec<LocalLabelId>> {
        tx.sync_bridge(move |tx| Self::store_labels(tx, labels))
            .await
    }

    #[instrument(skip_all)]
    pub fn store_labels(
        tx: &Transaction<'_>,
        labels: Vec<Label>,
    ) -> StashResult<Vec<LocalLabelId>> {
        let mut label_ids = Vec::with_capacity(labels.len());
        for mut label in labels {
            label.save_sync(tx)?;
            label_ids.push(label.id());
        }

        Ok(label_ids)
    }

    #[instrument(skip(api))]
    pub async fn patch_expanded<API: ProtonCore>(
        id: LabelId,
        expanded: bool,
        api: &API,
    ) -> Result<Label, ApiServiceError> {
        api.patch_label(
            id,
            PatchLabelRequest {
                expanded: Some(expanded),
                ..Default::default()
            },
        )
        .await
        .map(|r| r.label.into())
    }

    #[instrument(skip(tether))]
    pub async fn find_by_kind(kind: LabelType, tether: &Tether) -> Result<Vec<Self>, StashError> {
        Label::find(
            "WHERE label_type = ? ORDER BY display_order ASC",
            params![kind],
            tether,
        )
        .await
    }

    #[instrument(skip(tether))]
    pub async fn local_ids_by_kind(
        kind: LabelType,
        tether: &Tether,
    ) -> Result<Vec<LocalLabelId>, StashError> {
        Label::find_local_id_by(tether, "WHERE label_type = ?", params![kind]).await
    }

    #[instrument(skip(tether))]
    pub async fn find_by_kinds(
        kinds: &[LabelType],
        tether: &Tether,
    ) -> Result<Vec<Self>, StashError> {
        let placeholders = placeholders(kinds);
        Label::find(
            format!("WHERE label_type IN ({placeholders}) ORDER BY display_order ASC"),
            kinds.to_sql(),
            tether,
        )
        .await
    }

    #[instrument(skip_all)]
    pub async fn all_mail(tether: &Tether) -> Result<Vec<Self>, StashError> {
        Self::find_by_kinds(&MAIL_LABEL_TYPES, tether).await
    }

    #[instrument(skip_all)]
    pub async fn watch(mail_stash: &Stash<UserDb>) -> Result<WatcherHandle, StashError> {
        mail_stash
            .subscribe_to(|sender| Box::new(LabelWatcher { sender }))
            .await
    }

    #[instrument(skip(tether))]
    pub async fn resolve_remote_label_id(
        local_id: LocalLabelId,
        tether: &Tether,
    ) -> Result<LabelId, LabelError> {
        let Some(label_id) = Self::local_id_counterpart(local_id, tether).await? else {
            return Err(LabelError::CouldNotResolveRemoteLabel(local_id));
        };

        Ok(label_id)
    }

    #[instrument(skip(tether))]
    pub async fn resolve_local_label_id(
        label_id: LabelId,
        tether: &Tether,
    ) -> Result<LocalLabelId, LabelError> {
        let Some(label_id) = Self::remote_id_counterpart(label_id.clone(), tether).await? else {
            return Err(LabelError::CouldNotResolveLocalLabel(label_id));
        };
        Ok(label_id)
    }

    pub async fn handle_event(
        tx: &WriteTx<'_>,
        id: &LabelId,
        action: Action,
        label: Option<&mut Label>,
        changeset: &mut RebaseChangeSet,
    ) -> Result<(), StashError> {
        action
            .log_entry(id, async |remote_id| {
                Label::remote_id_counterpart(remote_id.clone(), tx)
                    .await
                    .unwrap_or_default()
                    .map(|v| v.as_u64())
            })
            .await;
        match action {
            Action::Delete => {
                tx.execute(
                    "DELETE FROM labels WHERE remote_id = ?",
                    params![id.clone()],
                )
                .await?;
            }
            Action::Create => {
                if let Some(label) = label {
                    label.save(tx).await?;
                    changeset.add(label.id());
                } else {
                    warn!("Received label create without label");
                }
            }
            Action::Update | Action::UpdateFlags => {
                if let Some(label) = label {
                    label.save(tx).await?;
                    changeset.add(label.id());
                } else {
                    warn!("Received label update without label");
                }
            }
        }
        Ok(())
    }
}

impl ModelHooks for Label {
    fn after_load(&mut self, conn: &Connection) -> Result<(), StashError> {
        if let Some(remote_id) = &self.remote_parent_id
            && self.local_parent_id.is_none()
        {
            self.local_parent_id = Self::remote_id_counterpart_sync(remote_id, conn)?;
        }

        Ok(())
    }

    fn after_save(&mut self, tx: &Transaction<'_>) -> Result<(), StashError> {
        self.local_parent_id = match &self.remote_parent_id {
            Some(parent_id) => {
                let res = Self::remote_id_counterpart_sync(parent_id, tx)?;
                if res.is_none() {
                    error!(
                        ?self.local_id,
                        ?self.remote_id,
                        ?self.remote_parent_id,
                        "Got a label with a missing parent",
                    );
                }
                res
            }

            None => None,
        };

        tx.execute(
            "UPDATE labels SET local_parent_id=? WHERE local_id=?",
            (self.local_parent_id, self.local_id),
        )?;

        Ok(())
    }

    fn before_save(&mut self, tx: &Transaction<'_>) -> mail_stash::stash::StashResult<()> {
        if let Some(remote_id) = &self.remote_id
            && let Some(label) = Label::find_first_sync("WHERE remote_id=?", (remote_id,), tx)?
        {
            self.local_parent_id = label.local_parent_id;
            self.local_id = label.local_id;
        }

        Ok(())
    }
}

pub struct LabelWatcher {
    sender: flume::Sender<()>,
}

impl TableObserver for LabelWatcher {
    fn tables(&self) -> Vec<String> {
        vec![Label::table_name().to_string()]
    }

    fn on_tables_changed(&self, _changed_tables: &BTreeSet<String>) {
        self.sender
            .send(())
            .inspect_err(|e| error!("Failed to send notification for LabelWatcher: {e:?}"))
            .ok();
    }
}

impl From<ApiLabel> for Label {
    fn from(value: ApiLabel) -> Self {
        Self {
            local_id: None,
            remote_id: Some(value.id),
            local_parent_id: None,
            remote_parent_id: value.parent_id,
            color: value.color.into(),
            display_order: value.order,
            display: value.display,
            expanded: value.expanded,
            label_type: value.label_type.into(),
            name: value.name,
            notify: value.notify,
            path: value.path,
            sticky: value.sticky,
            last_unseen_message: value.last_unseen_message,
        }
    }
}

impl Label {
    #[cfg(feature = "test-utils")]
    #[must_use]
    pub fn test_default() -> Self {
        Self {
            label_type: LabelType::Label,
            local_id: Option::default(),
            remote_id: Option::default(),
            local_parent_id: Option::default(),
            remote_parent_id: Option::default(),
            color: LabelColor::default(),
            display: Default::default(),
            expanded: Default::default(),
            name: String::default(),
            notify: Default::default(),
            display_order: Default::default(),
            path: Option::default(),
            sticky: Default::default(),
            last_unseen_message: Default::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{new_label_test_connection, random_string};
    use mail_core_api::services::proton::LabelType as ApiLabelType;
    use mail_shared_types::ModelExtension as _;
    use pretty_assertions::assert_eq;
    use std::cmp::Ordering;

    fn api_label(id: &str, parent_id: Option<&str>) -> ApiLabel {
        ApiLabel {
            id: id.into(),
            parent_id: parent_id.map(Into::into),
            ..ApiLabel::test_default()
        }
    }

    #[test]
    fn collect() {
        let labels = Label::topo_sort([
            api_label("a", None),
            api_label("b", Some("a")),
            api_label("c", Some("d")),
            api_label("d", None),
            api_label("e", Some("f")),
            api_label("f", Some("a")),
        ]);

        let labels: Vec<_> = labels
            .into_iter()
            .map(|label| label.remote_id.unwrap().to_string())
            .collect();

        let assert = |lhs: &str, ord: Ordering, rhs: &str| {
            let lhs_idx = labels.iter().find_position(|id| *id == lhs).unwrap();
            let rhs_idx = labels.iter().find_position(|id| *id == rhs).unwrap();

            assert_eq!(ord, lhs_idx.cmp(&rhs_idx));
        };

        assert("a", Ordering::Less, "b");
        assert("d", Ordering::Less, "c");
        assert("f", Ordering::Less, "e");
        assert("a", Ordering::Less, "f");
    }

    #[tokio::test]
    async fn test_remote_label_add() {
        let mut tether = new_label_test_connection().await.connection();
        let labels = test_labels();
        tether
            .write_tx::<_, _, StashError>(async |tx| {
                for label in labels.clone() {
                    Label::from(label).save(tx).await?;
                }
                Ok(())
            })
            .await
            .unwrap();
        compare_remote_labels_with_local(&tether, labels).await;
    }

    #[tokio::test]
    async fn test_remote_label_add_1_char_long_name() {
        let mut tether = new_label_test_connection().await.connection();
        let label = test_label(random_string(1).as_str());
        tether
            .write_tx::<_, _, StashError>(async |tx| Label::from(label.clone()).save(tx).await)
            .await
            .unwrap();
        compare_remote_label_with_local(&tether, label).await;
    }

    #[tokio::test]
    async fn test_remote_label_add_100_char_long_name() {
        let mut tether = new_label_test_connection().await.connection();
        let label = test_label(random_string(100).as_str());
        tether
            .write_tx(async |tx| Label::from(label.clone()).save(tx).await)
            .await
            .unwrap();
        compare_remote_label_with_local(&tether, label).await;
    }

    #[tokio::test]
    async fn test_remote_label_update() {
        let mut tether = new_label_test_connection().await.connection();
        tether
            .write_tx::<_, _, StashError>(async |tx| {
                tx.execute("DELETE FROM labels", vec![]).await?;
                Ok(())
            })
            .await
            .unwrap();
        let mut labels = test_labels()
            .into_iter()
            .map(Label::from)
            .collect::<Vec<_>>();
        let mut remote_labels = test_labels();
        tether
            .write_tx::<_, _, StashError>(async |tx| {
                for label in &mut labels {
                    label.save(tx).await.unwrap();
                }

                // Perform Some Updates
                remote_labels[0].color = "#xxxxx".into();
                remote_labels[0].name = "FooBar".into();
                remote_labels[1].sticky = true;
                remote_labels[1].expanded = true;
                remote_labels[1].notify = true;
                remote_labels[1].display = true;
                // Switch parents
                remote_labels[2].parent_id = Some(remote_labels[3].id.clone());
                remote_labels[2].order = 3;
                remote_labels[2].path = Some("Folder2/Folder1".to_owned());
                remote_labels[3].parent_id = None;
                remote_labels[3].path = None;
                remote_labels[3].order = 2;

                // Perform Some Updates
                labels[0].color = "#xxxxx".into();
                labels[0].name = "FooBar".into();
                labels[1].sticky = true;
                labels[1].expanded = true;
                labels[1].notify = true;
                labels[1].display = true;
                // Switch parents
                labels[2].remote_parent_id = labels[3].remote_id.clone();
                labels[2].display_order = 3;
                labels[2].path = Some("Folder2/Folder1".to_owned());
                labels[3].remote_parent_id = None;
                labels[3].path = None;
                labels[3].display_order = 2;

                for label in &mut labels {
                    label.save(tx).await?;
                }
                Ok(())
            })
            .await
            .unwrap();

        compare_remote_labels_with_local(&tether, remote_labels).await;
    }

    #[tokio::test]
    async fn test_delete_remote() {
        let mut tether = new_label_test_connection().await.connection();
        let mut labels = test_labels();

        tether
            .write_tx::<_, _, StashError>(async |tx| {
                for label in labels.clone() {
                    let mut label = Label::from(label);
                    if let Some(parent_id) = label.remote_parent_id.clone() {
                        label.local_parent_id = Label::find_by_remote_id(parent_id, tx)
                            .await
                            .expect("failed to get parent label")
                            .expect("parent label should exist")
                            .local_id;
                    }
                    label.save(tx).await.unwrap();
                }

                tx.execute(
                    "DELETE FROM labels WHERE remote_id = ?",
                    params![labels[0].id.clone()],
                )
                .await
                .expect("failed to delete local label");
                Ok(())
            })
            .await
            .unwrap();

        labels.remove(0);

        let remote_labels = labels;

        compare_remote_labels_with_local(&tether, remote_labels).await;
    }

    #[tokio::test]
    async fn create_local_label() {
        let mut tether = new_label_test_connection().await.connection();
        tether
            .write_tx::<_, _, StashError>(async |tx| {
                for t in [LabelType::Label, LabelType::Folder, LabelType::System] {
                    let mut new_label = Label {
                        remote_id: Some(format!("Label-{t:?}").into()),
                        color: LabelColor::purple(),
                        label_type: LabelType::Folder,
                        name: "Label".to_owned(),
                        ..Label::test_default()
                    };
                    new_label.save(tx).await.expect("failed to create label");
                    let db_label = Label::load(new_label.id(), tx)
                        .await
                        .expect("failed to load label")
                        .expect("should have a value");
                    assert_eq!(new_label, db_label, "Label of type {:?} does not match", t);
                }
                Ok(())
            })
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn create_local_label_1_char_long_name() {
        let mut tether = new_label_test_connection().await.connection();
        tether
            .write_tx::<_, _, StashError>(async |tx| {
                for t in [LabelType::Label, LabelType::Folder] {
                    let label_name = random_string(1);
                    let mut new_label = Label {
                        remote_id: Some(format!("Label-{t:?}").into()),
                        color: LabelColor::purple(),
                        label_type: LabelType::Folder,
                        name: label_name.clone(),
                        ..Label::test_default()
                    };
                    new_label.save(tx).await.expect("failed to create label");
                    let db_label = Label::load(new_label.id(), tx)
                        .await
                        .expect("failed to load label")
                        .expect("should have a value");
                    assert_eq!(new_label, db_label, "Label of type {:?} does not match", t);
                }
                Ok(())
            })
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn create_local_label_100_char_long_name() {
        let mut tether = new_label_test_connection().await.connection();
        tether
            .write_tx::<_, _, StashError>(async |tx| {
                for t in [LabelType::Label, LabelType::Folder] {
                    let label_name = random_string(100);
                    let mut new_label = Label {
                        remote_id: Some(format!("Label-{t:?}").into()),
                        color: LabelColor::purple(),
                        label_type: LabelType::Folder,
                        name: label_name.clone(),
                        ..Label::test_default()
                    };
                    new_label.save(tx).await.expect("failed to create label");
                    let db_label = Label::load(new_label.id(), tx)
                        .await
                        .expect("failed to load label")
                        .expect("should have a value");
                    assert_eq!(new_label, db_label, "Label of type {:?} does not match", t);
                }
                Ok(())
            })
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn update_local_label() {
        let mut tether = new_label_test_connection().await.connection();
        tether
            .write_tx::<_, _, StashError>(async |tx| {
                let mut new_label = Label {
                    remote_id: Some("MyLabel".into()),
                    color: LabelColor::purple(),
                    label_type: LabelType::Folder,
                    name: "Label".to_owned(),
                    ..Label::test_default()
                };
                new_label.save(tx).await.expect("failed to create label");
                let new_label2 = Label {
                    remote_id: Some("MyOtherLabel".into()),
                    color: LabelColor::purple(),
                    label_type: LabelType::Folder,
                    name: "Label".to_owned(),
                    ..Label::test_default()
                };
                new_label.save(tx).await.expect("failed to create label");

                new_label.color = LabelColor::black();
                new_label.save(tx).await.expect("failed to save label");
                compare_db_label(tx, new_label.id(), |l| {
                    assert_eq!(l.color, LabelColor::black());
                })
                .await;

                new_label.name = "NewName".to_owned();
                new_label.save(tx).await.expect("failed to save label");

                compare_db_label(tx, new_label.id(), |l| {
                    assert_eq!(l.name, "NewName");
                })
                .await;

                new_label.remote_parent_id = new_label2.remote_id.clone();
                new_label.path = Some("MyLabel/NewName".into());
                new_label.save(tx).await.expect("failed to save label");

                compare_db_label(tx, new_label.id(), |l| {
                    assert_eq!(l.remote_parent_id, new_label2.remote_id);
                    assert_eq!(l.path, Some("MyLabel/NewName".into()));
                })
                .await;
                Ok(())
            })
            .await
            .unwrap();
    }

    async fn compare_db_label(tx: &Tether, id: LocalLabelId, f: impl FnOnce(&Label)) {
        let db_label = Label::load(id, tx)
            .await
            .expect("failed to get label")
            .expect("must have value");
        (f)(&db_label);
    }

    #[tokio::test]
    async fn test_watch_label() {
        let mail_stash = new_label_test_connection().await;
        let mut tether = mail_stash.connection();
        let mut label = tether
            .write_tx::<_, _, StashError>(async |tx| {
                let mut label: Label = ApiLabel {
                    id: LabelId::from("label_id"),
                    name: "MyLabel".to_owned(),
                    color: "#ffffff".to_owned(),
                    display: true,
                    expanded: true,
                    ..ApiLabel::test_default()
                }
                .into();

                label.save(tx).await.unwrap();
                Ok(label)
            })
            .await
            .unwrap();

        let db_label = Label::load(label.id(), &tether).await.unwrap().unwrap();
        let handle = Label::watch(&mail_stash).await.unwrap();
        let watcher = &handle.receiver;

        assert_eq!(db_label, label);

        label.display_order = 10;
        tether
            .write_tx::<_, _, StashError>(async |tx| label.save(tx).await)
            .await
            .unwrap();

        watcher.recv_async().await.unwrap();
    }

    async fn compare_remote_labels_with_local(tether: &Tether, remote_labels: Vec<ApiLabel>) {
        for remote_label in remote_labels {
            compare_remote_label_with_local(tether, remote_label).await;
        }
    }

    async fn compare_remote_label_with_local(tether: &Tether, remote_label: ApiLabel) {
        let local_labels = Label::all(tether).await.expect("failed to get labels");
        let find_label = |id: &LabelId| -> &Label {
            local_labels
                .iter()
                .find(|l| l.remote_id == Some(id.clone()))
                .expect("failed to find local label")
        };

        let local = find_label(&remote_label.id);
        compare_local_to_remote(tether, local, &remote_label).await;
    }

    fn test_labels() -> Vec<ApiLabel> {
        vec![
            ApiLabel {
                id: LabelId::from("label_id"),
                name: "MyLabel".to_owned(),
                color: "#ffffff".to_owned(),
                label_type: ApiLabelType::Label,
                display: true,
                expanded: true,
                ..ApiLabel::test_default()
            },
            ApiLabel {
                id: LabelId::from("50"),
                name: "Inbox2".to_owned(),
                color: "#ffffff".to_owned(),
                label_type: ApiLabelType::System,
                notify: true,
                sticky: true,
                ..ApiLabel::test_default()
            },
            ApiLabel {
                id: LabelId::from("Folder1"),
                name: "Folder1".to_owned(),
                color: "#ffffff".to_owned(),
                label_type: ApiLabelType::Folder,
                notify: true,
                display: true,
                order: 2,
                ..ApiLabel::test_default()
            },
            ApiLabel {
                id: LabelId::from("Folder2"),
                parent_id: Some(LabelId::from("Folder1")),
                name: "Folder2".to_owned(),
                path: Some("Folder1/Folder2".to_owned()),
                color: "#ffffff".to_owned(),
                label_type: ApiLabelType::Folder,
                sticky: true,
                expanded: true,
                order: 3,
                ..ApiLabel::test_default()
            },
        ]
    }

    fn test_label(name: &str) -> ApiLabel {
        ApiLabel {
            id: LabelId::from("label_id"),
            name: name.to_owned(),
            color: "#ffffff".to_owned(),
            label_type: ApiLabelType::Label,
            display: true,
            expanded: true,
            ..ApiLabel::test_default()
        }
    }

    async fn compare_local_to_remote(tether: &Tether, local: &Label, remote: &ApiLabel) {
        assert_eq!(
            local.remote_id,
            Some(remote.id.clone()),
            "remote id does not match for {}",
            remote.id
        );
        assert_eq!(
            local.remote_parent_id.is_some(),
            remote.parent_id.is_some(),
            "parent id state does not match for {}",
            remote.id
        );
        assert_eq!(
            local.name, remote.name,
            "name does not match for {}",
            remote.id
        );
        assert_eq!(
            local.path, remote.path,
            "path does not match for {}",
            remote.id
        );
        assert_eq!(
            local.color.to_string(),
            remote.color,
            "color does not match for {}",
            remote.id
        );
        assert_eq!(
            local.label_type,
            remote.label_type.into(),
            "label type does not match for {}",
            remote.id
        );
        assert_eq!(
            local.display_order, remote.order,
            "order does not match for {}",
            remote.id
        );
        let sticky: bool = remote.sticky;
        assert_eq!(
            local.sticky, sticky,
            "sticky does not match for {}",
            remote.id
        );

        let expanded: bool = remote.expanded;
        assert_eq!(
            local.expanded, expanded,
            "expanded does not match for {}",
            remote.id
        );
        let notify: bool = remote.notify;
        assert_eq!(
            local.notify, notify,
            "notified does not match for {}",
            remote.id
        );

        if let Some(remote_parent_id) = local.remote_parent_id.clone() {
            let parent_label = Label::find_by_remote_id(remote_parent_id, tether)
                .await
                .expect("failed to find parent label")
                .expect("Parent label should exist");
            assert_eq!(
                parent_label.remote_id.unwrap(),
                remote.parent_id.clone().unwrap(),
                "parent id value does not match for {}",
                remote.id
            );
        }
    }
}
