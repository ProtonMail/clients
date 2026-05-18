use std::fmt::{Display, Formatter};

use indoc::indoc;
use mail_action_queue::rebase::RebaseChangeSet;
use mail_contacts_api::{ContactApi, ContactGroup as ApiContactGroup, ContactGroupId};
use mail_core_api::service::ApiServiceError;
use mail_shared_types::{Action, ModelIdExtension};
use mail_stash::exports::{FromSql, FromSqlResult, SqliteError, ToSql, ToSqlOutput, ValueRef};
use mail_stash::macros::Model;
use mail_stash::orm::Model;
use mail_stash::stash::{StashError, WriteTx};
use mail_stash::{UserDb, rusqlite};
use serde::{Deserialize, Serialize};
use tracing::{instrument, warn};

use crate::local_ids::{LocalContactEmailId, LocalContactGroupId, LocalContactId};

#[derive(
    Clone,
    Debug,
    Default,
    Deserialize,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    Serialize
)]
pub struct ContactGroupColor(String);

impl ContactGroupColor {
    #[must_use]
    pub fn purple() -> Self {
        Self("#8080FF".into())
    }
    #[must_use]
    pub fn black() -> Self {
        Self("#000000".into())
    }

    #[must_use]
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl Display for ContactGroupColor {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for ContactGroupColor {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for ContactGroupColor {
    fn from(value: &str) -> Self {
        Self(value.into())
    }
}

impl FromSql for ContactGroupColor {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        value.as_str().map(|s| ContactGroupColor(s.to_string()))
    }
}

impl ToSql for ContactGroupColor {
    fn to_sql(&self) -> Result<ToSqlOutput<'_>, SqliteError> {
        Ok(ToSqlOutput::from(self.0.clone()))
    }
}

#[derive(Clone, Debug, Eq, Model, PartialEq)]
#[TableName("contact_group")]
#[Database(UserDb)]
pub struct ContactGroup {
    #[IdField(autoincrement)]
    pub local_id: Option<LocalContactGroupId>,

    #[DbField]
    pub remote_id: Option<ContactGroupId>,

    #[DbField]
    pub color: ContactGroupColor,

    #[DbField]
    pub display: bool,

    #[DbField]
    pub name: String,

    #[DbField]
    pub display_order: u32,

    #[DbField]
    pub sticky: bool,
}

impl ModelIdExtension for ContactGroup {
    type RemoteId = ContactGroupId;

    fn remote_id(&self) -> Option<&Self::RemoteId> {
        self.remote_id.as_ref()
    }
}

impl From<ApiContactGroup> for ContactGroup {
    fn from(value: ApiContactGroup) -> Self {
        Self {
            local_id: None,
            remote_id: Some(value.id),
            display: value.display,
            color: ContactGroupColor::from(value.color),
            name: value.name,
            display_order: value.order,
            sticky: value.sticky,
        }
    }
}

impl ContactGroup {
    #[cfg(feature = "test-utils")]
    pub fn test_default() -> Self {
        Self {
            local_id: None,
            remote_id: Some(ContactGroupId::from("ContactGroupID")),
            color: ContactGroupColor::black(),
            display: true,
            name: "ContactGroupName".into(),
            display_order: 0,
            sticky: false,
        }
    }

    #[instrument(skip_all)]
    pub async fn fetch<API>(api: &API) -> Result<Vec<Self>, ApiServiceError>
    where
        API: ContactApi,
    {
        api.get_contact_groups()
            .await
            .map(|v| v.labels.into_iter().map(Into::into).collect())
    }

    pub async fn handle_event(
        tx: &WriteTx<'_>,
        id: &ContactGroupId,
        action: Action,
        label: Option<&mut ContactGroup>,
        changeset: &mut RebaseChangeSet,
    ) -> Result<(), StashError> {
        action
            .log_entry(id, async |remote_id| {
                ContactGroup::remote_id_counterpart(remote_id.clone(), tx)
                    .await
                    .unwrap_or_default()
                    .map(|v| v.as_u64())
            })
            .await;
        match action {
            Action::Delete => {
                ContactGroup::delete_by_remote_id(id.clone(), tx).await?;
            }
            Action::Create => {
                if let Some(contact_group) = label {
                    contact_group.save(tx).await?;
                    changeset.add(contact_group.id());
                } else {
                    warn!("Received contact group create without label");
                }
            }
            Action::Update | Action::UpdateFlags => {
                if let Some(label) = label {
                    label.save(tx).await?;
                    changeset.add(label.id());
                } else {
                    warn!("Received contact_group update without label");
                }
            }
        }
        Ok(())
    }

    pub(crate) fn link_contact_groups_for_contact_email(
        tx: &rusqlite::Transaction<'_>,
        contat_email_id: LocalContactEmailId,
        contact_group_ids: &[ContactGroupId],
    ) -> Result<(), rusqlite::Error> {
        if contact_group_ids.is_empty() {
            return Ok(());
        }
        let mut contact_email_groups_stmt = tx.prepare_cached(LINK_CONTACT_GROUPS_EMAILS_QUERY)?;

        for contact_group_id in contact_group_ids {
            contact_email_groups_stmt
                .execute(rusqlite::params![contat_email_id, contact_group_id])?;
        }

        Ok(())
    }

    pub(crate) fn link_contact_groups_for_contact_email_with_local_id(
        tx: &rusqlite::Transaction<'_>,
        contact_email_id: LocalContactEmailId,
        contact_group_ids: &[LocalContactGroupId],
    ) -> Result<(), rusqlite::Error> {
        if contact_group_ids.is_empty() {
            return Ok(());
        }
        let mut contact_email_groups_stmt =
            tx.prepare_cached("INSERT OR IGNORE INTO contact_email_groups (local_contact_email_id, local_contact_group_id) VALUES (?,?)")?;

        for contact_group_id in contact_group_ids {
            contact_email_groups_stmt
                .execute(rusqlite::params![contact_email_id, contact_group_id])?;
        }
        Ok(())
    }

    pub(crate) fn relink_contact_groups_for_contact_email(
        tx: &rusqlite::Transaction<'_>,
        contact_email_id: LocalContactEmailId,
        contact_group_ids: &[ContactGroupId],
    ) -> Result<(), rusqlite::Error> {
        let mut stmt =
            tx.prepare_cached("DELETE FROM contact_email_groups WHERE local_contact_email_id = ?")?;
        stmt.execute(rusqlite::params![contact_email_id])?;
        Self::link_contact_groups_for_contact_email(tx, contact_email_id, contact_group_ids)
    }

    pub(crate) fn relink_contact_groups_for_contact_email_local_id(
        tx: &rusqlite::Transaction<'_>,
        contact_email_id: LocalContactEmailId,
        contact_group_ids: &[LocalContactGroupId],
    ) -> Result<(), rusqlite::Error> {
        let mut stmt =
            tx.prepare_cached("DELETE FROM contact_email_groups WHERE local_contact_email_id = ?")?;
        stmt.execute(rusqlite::params![contact_email_id])?;

        Self::link_contact_groups_for_contact_email_with_local_id(
            tx,
            contact_email_id,
            contact_group_ids,
        )
    }

    pub(crate) fn relink_contact_groups_for_contact(
        tx: &rusqlite::Transaction<'_>,
        contact_id: LocalContactId,
        contact_group_ids: &[ContactGroupId],
    ) -> Result<(), rusqlite::Error> {
        let mut stmt =
            tx.prepare_cached("DELETE FROM contact_contact_groups WHERE local_contact_id = ?")?;

        stmt.execute(rusqlite::params![contact_id])?;

        Self::link_contact_groups_for_contact(tx, contact_id, contact_group_ids)
    }

    pub(crate) fn relink_contact_groups_for_contact_local_id(
        tx: &rusqlite::Transaction<'_>,
        contact_id: LocalContactId,
        contact_group_ids: &[LocalContactGroupId],
    ) -> Result<(), rusqlite::Error> {
        let mut stmt =
            tx.prepare_cached("DELETE FROM contact_contact_groups WHERE local_contact_id = ?")?;

        stmt.execute(rusqlite::params![contact_id])?;

        Self::link_contact_groups_for_contact_local_id(tx, contact_id, contact_group_ids)
    }

    pub(crate) fn link_contact_groups_for_contact(
        tx: &rusqlite::Transaction<'_>,
        contact_id: LocalContactId,
        contact_group_ids: &[ContactGroupId],
    ) -> Result<(), rusqlite::Error> {
        if contact_group_ids.is_empty() {
            return Ok(());
        }

        let mut contact_email_groups_stmt =
            tx.prepare_cached(LINK_CONTACT_GROUPS_CONTATCS_QUERY)?;

        for contact_group_id in contact_group_ids {
            contact_email_groups_stmt.execute(rusqlite::params![contact_id, contact_group_id])?;
        }

        Ok(())
    }

    pub(crate) fn link_contact_groups_for_contact_local_id(
        tx: &rusqlite::Transaction<'_>,
        contact_id: LocalContactId,
        contact_group_ids: &[LocalContactGroupId],
    ) -> Result<(), rusqlite::Error> {
        if contact_group_ids.is_empty() {
            return Ok(());
        }

        let mut contact_email_groups_stmt =
            tx.prepare_cached("INSERT OR IGNORE INTO contact_contact_groups (local_contact_id, local_contact_group_id) VALUES (?,?)")?;

        for contact_group_id in contact_group_ids {
            contact_email_groups_stmt.execute(rusqlite::params![contact_id, contact_group_id])?;
        }

        Ok(())
    }
}

pub(crate) const LINK_CONTACT_GROUPS_CONTATCS_QUERY: &str = indoc! {
    "INSERT OR IGNORE INTO contact_contact_groups (local_contact_id, local_contact_group_id)
    SELECT ? AS local_contact_id , local_id as local_contact_group_id FROM contact_group WHERE contact_group.remote_id =?",
};

pub(crate) const LINK_CONTACT_GROUPS_EMAILS_QUERY: &str = indoc! {"
    INSERT OR IGNORE INTO contact_email_groups (local_contact_email_id, local_contact_group_id)
    SELECT ? AS local_contact_email_id , local_id as local_contact_group_id FROM contact_group WHERE contact_group.remote_id =?",
};
