use std::vec;

use crate::contact::Contact;
use crate::contact_group::ContactGroup;
use crate::database::{ContactReadTx, ContactWriteTx};
use crate::local_ids::LocalContactId;
use contact_database::{RoContactTable, RwContactTable};
use mail_contacts_api::ContactId;
use mail_shared_types::ModelExtension;
use mail_stash::exports::Connection;
use mail_stash::orm::{Model, ModelRaw};
use mail_stash::rusqlite::params_from_iter;
use mail_stash::stash::StashError;
use mail_stash::utils::placeholders;

impl RoContactTable for ContactReadTx<'_> {
    type Error = StashError;

    async fn find_contact_by_id(
        &self,
        id: contact_database::LocalContactId,
    ) -> Result<Option<contact_database::Contact>, Self::Error> {
        self.0
            .sync_query(move |conn| find_contact_by_id(conn, id.into()))
            .await
    }

    async fn find_contact_by_remote_id(
        &self,
        id: &mail_contacts_api::ContactId,
    ) -> Result<Option<contact_database::Contact>, Self::Error> {
        let id = id.clone();
        self.0
            .sync_query(move |conn| find_contact_by_remote_id(conn, &id))
            .await
    }

    async fn find_contacts_by_ids(
        &self,
        ids: impl IntoIterator<Item = contact_database::LocalContactId>,
    ) -> Result<Vec<contact_database::Contact>, Self::Error> {
        let ids = ids.into_iter().map(Into::into).collect();
        self.0
            .sync_query(move |conn| find_contact_by_ids(conn, ids))
            .await
    }

    async fn find_contact_by_remote_ids(
        &self,
        ids: impl IntoIterator<Item = ContactId>,
    ) -> Result<Vec<contact_database::Contact>, Self::Error> {
        let ids = ids.into_iter().collect::<Vec<_>>();
        self.0
            .sync_query(move |conn| find_contact_by_remote_ids(conn, &ids))
            .await
    }
}

impl RoContactTable for ContactWriteTx<'_> {
    type Error = StashError;

    async fn find_contact_by_id(
        &self,
        id: contact_database::LocalContactId,
    ) -> Result<Option<contact_database::Contact>, Self::Error> {
        self.0
            .sync_query(move |conn| find_contact_by_id(conn, id.into()))
            .await
    }

    async fn find_contact_by_remote_id(
        &self,
        id: &ContactId,
    ) -> Result<Option<contact_database::Contact>, Self::Error> {
        let id = id.clone();
        self.0
            .sync_query(move |conn| find_contact_by_remote_id(conn, &id))
            .await
    }

    async fn find_contacts_by_ids(
        &self,
        ids: impl IntoIterator<Item = contact_database::LocalContactId>,
    ) -> Result<Vec<contact_database::Contact>, Self::Error> {
        let ids = ids.into_iter().map(Into::into).collect();
        self.0
            .sync_query(move |conn| find_contact_by_ids(conn, ids))
            .await
    }

    async fn find_contact_by_remote_ids(
        &self,
        ids: impl IntoIterator<Item = ContactId>,
    ) -> Result<Vec<contact_database::Contact>, Self::Error> {
        let ids = ids.into_iter().collect::<Vec<_>>();
        self.0
            .sync_query(move |conn| find_contact_by_remote_ids(conn, &ids))
            .await
    }
}

impl RwContactTable for ContactWriteTx<'_> {
    async fn create_contact(
        &self,
        contact: contact_database::NewContact,
    ) -> Result<contact_database::Contact, Self::Error> {
        self.0
            .sync_bridge(move |tx| {
                let label_ids = contact.label_ids;
                let mut contact = Contact {
                    local_id: None,
                    remote_id: None,
                    cards: vec![],
                    contact_emails: vec![],
                    create_time: contact.create_time,
                    label_ids: vec![],
                    modify_time: contact.modify_time,
                    name: contact.name,
                    size: contact.size,
                    uid: contact.uid,
                    deleted: false,
                };
                contact.save_raw_sync(tx)?;
                ContactGroup::link_contact_groups_for_contact(tx, contact.id(), &label_ids)?;

                let mut contact: contact_database::Contact = contact.into();
                load_contact_label_ids(tx, std::slice::from_mut(&mut contact))?;
                Ok(contact)
            })
            .await
    }

    async fn upsert_contact(
        &self,
        contact: contact_database::UpsertableContact,
    ) -> Result<contact_database::Contact, Self::Error> {
        self.0
            .sync_bridge(move |tx| {
                let mut contact: Contact = contact.into();
                contact.save_raw_sync(tx)?;
                ContactGroup::relink_contact_groups_for_contact(
                    tx,
                    contact.id(),
                    &contact.label_ids,
                )?;

                let mut contact: contact_database::Contact = contact.into();
                load_contact_label_ids(tx, std::slice::from_mut(&mut contact))?;
                Ok(contact)
            })
            .await
    }

    async fn upsert_contacts(
        &self,
        contacts: impl IntoIterator<Item = contact_database::UpsertableContact>,
    ) -> Result<Vec<contact_database::Contact>, Self::Error> {
        let contacts: Vec<Contact> = contacts.into_iter().map(Into::into).collect::<Vec<_>>();
        self.0
            .sync_bridge(move |tx| {
                let mut result = Vec::with_capacity(contacts.len());
                for mut contact in contacts {
                    contact.save_raw_sync(tx)?;
                    ContactGroup::relink_contact_groups_for_contact(
                        tx,
                        contact.id(),
                        &contact.label_ids,
                    )?;

                    let mut contact: contact_database::Contact = contact.into();
                    load_contact_label_ids(tx, std::slice::from_mut(&mut contact))?;
                    result.push(contact);
                }

                Ok(result)
            })
            .await
    }

    async fn update_contact(&self, contact: &contact_database::Contact) -> Result<(), Self::Error> {
        let contact_group_ids = contact
            .label_ids
            .iter()
            .map(|v| (*v).into())
            .collect::<Vec<_>>();
        let mut contact: Contact = contact.into();
        self.0
            .sync_bridge(move |tx| {
                contact.save_raw_sync(tx)?;
                ContactGroup::relink_contact_groups_for_contact_local_id(
                    tx,
                    contact.id(),
                    &contact_group_ids,
                )?;
                Ok(())
            })
            .await
    }

    async fn mark_contact_as_deleted(
        &self,
        ids: impl IntoIterator<Item = contact_database::LocalContactId>,
    ) -> Result<(), Self::Error> {
        let contact_ids: Vec<LocalContactId> = ids.into_iter().map(Into::into).collect();
        self.0
            .sync_bridge(move |tx| {
                let mut stmt =
                    tx.prepare_cached("UPDATE contact SET deleted = 1 WHERE local_id = ?")?;
                for contact_id in contact_ids {
                    stmt.execute([contact_id])?;
                }
                Ok(())
            })
            .await?;
        Ok(())
    }

    async fn mark_contact_as_undeleted(
        &self,
        ids: impl IntoIterator<Item = contact_database::LocalContactId>,
    ) -> Result<(), Self::Error> {
        let contact_ids: Vec<LocalContactId> = ids.into_iter().map(Into::into).collect();
        self.0
            .sync_bridge(move |tx| {
                let mut stmt =
                    tx.prepare_cached("UPDATE contact SET deleted = 0 WHERE local_id = ?")?;
                for contact_id in contact_ids {
                    stmt.execute([contact_id])?;
                }
                Ok(())
            })
            .await?;
        Ok(())
    }

    async fn delete_contacts(
        &self,
        ids: impl IntoIterator<Item = contact_database::LocalContactId>,
    ) -> Result<(), Self::Error> {
        let contact_ids: Vec<LocalContactId> = ids.into_iter().map(Into::into).collect();
        self.0
            .sync_bridge(move |tx| Contact::delete_by_ids_sync(&contact_ids, tx))
            .await?;
        Ok(())
    }
}

impl From<Contact> for contact_database::Contact {
    fn from(value: Contact) -> Self {
        let local_id = value.id();
        Self {
            local_id: local_id.into(),
            remote_id: value.remote_id,
            create_time: value.create_time,
            label_ids: vec![],
            modify_time: value.modify_time,
            name: value.name,
            size: value.size,
            uid: value.uid,
            deleted: value.deleted,
        }
    }
}

impl From<contact_database::Contact> for Contact {
    fn from(value: contact_database::Contact) -> Self {
        Self {
            local_id: Some(value.local_id.into()),
            remote_id: value.remote_id,
            create_time: value.create_time,
            label_ids: vec![],
            modify_time: value.modify_time,
            name: value.name,
            size: value.size,
            uid: value.uid,
            deleted: value.deleted,
            cards: vec![],
            contact_emails: vec![],
        }
    }
}

impl From<&contact_database::Contact> for Contact {
    fn from(value: &contact_database::Contact) -> Self {
        Self {
            local_id: Some(value.local_id.into()),
            remote_id: value.remote_id.clone(),
            create_time: value.create_time,
            label_ids: vec![],
            modify_time: value.modify_time,
            name: value.name.clone(),
            size: value.size,
            uid: value.uid.clone(),
            deleted: value.deleted,
            cards: vec![],
            contact_emails: vec![],
        }
    }
}

impl From<contact_database::UpsertableContact> for Contact {
    fn from(value: contact_database::UpsertableContact) -> Self {
        Self {
            local_id: None,
            remote_id: Some(value.id),
            cards: vec![],
            contact_emails: vec![],
            create_time: value.create_time,
            label_ids: value.label_ids,
            modify_time: value.modify_time,
            name: value.name,
            size: value.size,
            uid: value.uid,
            deleted: false,
        }
    }
}

fn find_contact_by_id(
    conn: &Connection,
    local_id: LocalContactId,
) -> Result<Option<contact_database::Contact>, StashError> {
    let mut contact: Option<contact_database::Contact> =
        Contact::find_first_raw_sync("WHERE local_id =?", [local_id], conn)?.map(Into::into);
    if let Some(contact) = contact.as_mut() {
        load_contact_label_ids(conn, std::slice::from_mut(contact))?;
    }
    Ok(contact)
}

fn find_contact_by_remote_id(
    conn: &Connection,
    remote_id: &ContactId,
) -> Result<Option<contact_database::Contact>, StashError> {
    let mut contact: Option<contact_database::Contact> =
        Contact::find_first_raw_sync("WHERE remote_id =?", [remote_id], conn)?.map(Into::into);
    if let Some(contact) = contact.as_mut() {
        load_contact_label_ids(conn, std::slice::from_mut(contact))?;
    }
    Ok(contact)
}

fn find_contact_by_ids(
    conn: &Connection,
    local_ids: Vec<LocalContactId>,
) -> Result<Vec<contact_database::Contact>, StashError> {
    let mut contacts: Vec<contact_database::Contact> = Contact::find_raw_sync(
        format!("WHERE local_id IN ({})", placeholders(&local_ids)),
        params_from_iter(local_ids),
        conn,
    )?
    .into_iter()
    .map(Into::into)
    .collect();
    load_contact_label_ids(conn, &mut contacts)?;
    Ok(contacts)
}

fn find_contact_by_remote_ids(
    conn: &Connection,
    remote_ids: &[ContactId],
) -> Result<Vec<contact_database::Contact>, StashError> {
    let mut contacts: Vec<contact_database::Contact> = Contact::find_raw_sync(
        format!("WHERE remote_id IN ({})", placeholders(remote_ids)),
        params_from_iter(remote_ids),
        conn,
    )?
    .into_iter()
    .map(Into::into)
    .collect();
    load_contact_label_ids(conn, &mut contacts)?;
    Ok(contacts)
}

fn load_contact_label_ids(
    conn: &Connection,
    contacts: &mut [contact_database::Contact],
) -> Result<(), StashError> {
    let mut stmt = conn.prepare_cached(
        "SELECT local_contact_group_id FROM contact_contact_groups WHERE local_contact_id = ?",
    )?;
    for contact in contacts {
        let rows = stmt.query_map([contact.local_id.as_u64()], |r| {
            r.get::<usize, u64>(0)
                .map(contact_database::LocalContactGroupId::from)
        })?;

        let mut label_ids = Vec::with_capacity(rows.size_hint().0);
        for row in rows {
            label_ids.push(row?);
        }
        contact.label_ids = label_ids;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::database::ContactStashDb;
    use crate::test_utils::new_contact_test_connection;
    use contact_database::LocalContactId;
    use contact_service::ContactRepository;
    use mail_db::Database;

    async fn new_db() -> ContactStashDb {
        ContactStashDb(mail_db_stash::StashDb::new(
            new_contact_test_connection().await,
        ))
    }

    #[tokio::test]
    async fn find_contact_by_id() {
        let db = new_db().await;
        db.read(async |tx| {
            ContactRepository::find_contact_by_id(&tx, LocalContactId::from(10)).await
        })
        .await
        .unwrap();
    }
}
