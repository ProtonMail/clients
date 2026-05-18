use contact_database::{RoContactGroupTable, RwContactGroupTable};
use mail_shared_types::{ModelExtension, ModelIdExtension};
use mail_stash::orm::Model;
use mail_stash::stash::StashError;

use crate::contact_group::ContactGroup;
use crate::database::{ContactReadTx, ContactWriteTx};

impl RoContactGroupTable for ContactReadTx<'_> {
    type Error = StashError;

    async fn find_contact_group_by_id(
        &self,
        id: contact_database::LocalContactGroupId,
    ) -> Result<Option<contact_database::ContactGroup>, Self::Error> {
        Ok(ContactGroup::find_by_id(id.into(), &self.0)
            .await?
            .map(Into::into))
    }

    async fn find_contact_group_by_remote_id(
        &self,
        id: &mail_contacts_api::ContactGroupId,
    ) -> Result<Option<contact_database::ContactGroup>, Self::Error> {
        Ok(ContactGroup::find_by_remote_id(id.clone(), &self.0)
            .await?
            .map(Into::into))
    }
}

impl RoContactGroupTable for ContactWriteTx<'_> {
    type Error = StashError;

    async fn find_contact_group_by_id(
        &self,
        id: contact_database::LocalContactGroupId,
    ) -> Result<Option<contact_database::ContactGroup>, Self::Error> {
        Ok(ContactGroup::find_by_id(id.into(), &self.0)
            .await?
            .map(Into::into))
    }

    async fn find_contact_group_by_remote_id(
        &self,
        id: &mail_contacts_api::ContactGroupId,
    ) -> Result<Option<contact_database::ContactGroup>, Self::Error> {
        Ok(ContactGroup::find_by_remote_id(id.clone(), &self.0)
            .await?
            .map(Into::into))
    }
}

impl RwContactGroupTable for ContactWriteTx<'_> {
    async fn create_contact_group(
        &self,
        contact_group: contact_database::NewContactGroup,
    ) -> Result<contact_database::ContactGroup, Self::Error> {
        self.0
            .sync_bridge(move |tx| {
                let mut contact_group: ContactGroup = contact_group.into();
                contact_group.save_sync(tx)?;
                Ok(contact_group.into())
            })
            .await
    }

    async fn upsert_contact_group(
        &self,
        contact_group: contact_database::UpsertableContactGroup,
    ) -> Result<contact_database::ContactGroup, Self::Error> {
        self.0
            .sync_bridge(move |tx| {
                let mut contact_group: ContactGroup = contact_group.into();
                contact_group.save_sync(tx)?;
                Ok(contact_group.into())
            })
            .await
    }

    async fn upsert_contact_groups(
        &self,
        contact_groups: impl IntoIterator<Item = contact_database::UpsertableContactGroup>,
    ) -> Result<Vec<contact_database::ContactGroup>, Self::Error> {
        let mut contact_groups: Vec<ContactGroup> =
            contact_groups.into_iter().map(Into::into).collect();
        self.0
            .sync_bridge(move |tx| {
                for contact_group in &mut contact_groups {
                    contact_group.save_sync(tx)?;
                }
                Ok(contact_groups.into_iter().map(Into::into).collect())
            })
            .await
    }

    async fn update_contact_group(
        &self,
        contact_group: &contact_database::ContactGroup,
    ) -> Result<(), Self::Error> {
        let mut contact_group: ContactGroup = contact_group.into();
        self.0
            .sync_bridge(move |tx| {
                contact_group.save_sync(tx)?;
                Ok(())
            })
            .await
    }

    async fn delete_contact_groups(
        &self,
        ids: impl IntoIterator<Item = contact_database::LocalContactGroupId>,
    ) -> Result<(), Self::Error> {
        let ids = ids.into_iter().map(Into::into).collect();
        ContactGroup::delete_by_ids(ids, &self.0).await?;
        Ok(())
    }
}

impl From<ContactGroup> for contact_database::ContactGroup {
    fn from(value: ContactGroup) -> Self {
        Self {
            local_id: value.id().into(),
            remote_id: value.remote_id,
            color: value.color.into_inner(),
            display: value.display,
            name: value.name,
            order: value.display_order,
            sticky: value.sticky,
        }
    }
}

impl From<contact_database::UpsertableContactGroup> for ContactGroup {
    fn from(value: contact_database::UpsertableContactGroup) -> Self {
        Self {
            local_id: None,
            remote_id: Some(value.id),
            color: value.color.into(),
            display: value.display,
            name: value.name,
            display_order: value.order,
            sticky: value.sticky,
        }
    }
}

impl From<contact_database::NewContactGroup> for ContactGroup {
    fn from(value: contact_database::NewContactGroup) -> Self {
        Self {
            local_id: None,
            remote_id: None,
            color: value.color.into(),
            display: value.display,
            name: value.name,
            display_order: value.order,
            sticky: value.sticky,
        }
    }
}

impl From<&contact_database::ContactGroup> for ContactGroup {
    fn from(value: &contact_database::ContactGroup) -> Self {
        Self {
            local_id: Some(value.local_id.into()),
            remote_id: value.remote_id.clone(),
            color: value.color.clone().into(),
            display: value.display,
            name: value.name.clone(),
            display_order: value.order,
            sticky: value.sticky,
        }
    }
}
