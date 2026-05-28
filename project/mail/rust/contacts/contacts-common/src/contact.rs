use std::collections::{BTreeSet, HashMap};
use std::default::Default;
use std::io::Cursor;
use std::time::Instant;

use anyhow::Context;
use bytes::Buf as _;
use futures::future::try_join_all;
use futures::try_join;
use ical::VcardParser;
use indoc::indoc;
use itertools::Itertools;
use mail_action_queue::rebase::RebaseChangeSet;
use mail_api_utils::Paginatable;
use mail_core_api::SYNC_CONTACT_PAGE_SIZE;
use mail_core_api::service::ApiServiceError;
use mail_core_api::services::proton::{
    ContactBasic as ApiContactBasic, ContactEmail as ApiContactEmail,
    ContactFull as ApiContactFull, ContactId, ContactUID, GetContactsEmailsOptions,
    GetContactsEmailsResponse, GetContactsOptions, GetContactsResponse,
};
use mail_core_api::session::Session;
use mail_shared_types::{InitializationKey, MapVec, ModelExtension, ModelIdExtension};
use mail_stash::exports::Transaction;
use mail_stash::macros::{Model as ModelDerive, ModelRaw};
use mail_stash::orm::{DbRecord, Model, ModelHooks};
use mail_stash::rusqlite::{Connection, params_from_iter};
use mail_stash::stash::{
    RunTransaction, Stash, StashError, StashResult, Tether, WatcherHandle, WriteTx,
};
use mail_stash::utils::{ConnectionExt, placeholders};
use mail_stash::{UserDb, params, rusqlite};
use mail_vcard::vcard::{PropertyUid, VCard};
use proton_crypto::crypto::PGPProviderSync;
use proton_crypto_account::contacts::DecryptableVerifiableCard as _;
use proton_crypto_account::keys::UnlockedUserKeys;
use sqlite_watcher::watcher::TableObserver;
use thiserror::Error;
use tracing::{debug, error, info};

use crate::contact_card::ContactCard;
use crate::contact_email::ContactEmail;
use crate::contact_group::{
    ContactGroup, LINK_CONTACT_GROUPS_CONTATCS_QUERY, LINK_CONTACT_GROUPS_EMAILS_QUERY,
};
use crate::contact_list::{
    ContactGroupItem, ContactSuggestions, DeviceContact, GroupedContacts, build_grouped_contacts,
    email_item_from_mail,
};
use crate::error::ContactError;
use crate::local_ids::{LocalContactEmailId, LocalContactGroupId, LocalContactId};
use mail_contacts_api::{ContactApi as _, ContactGroupId};

#[derive(Clone, Debug, Eq, ModelDerive, PartialEq, ModelRaw)]
#[TableName("contacts")]
#[Database(UserDb)]
#[ModelHooks]
pub struct Contact {
    #[IdField(autoincrement)]
    pub local_id: Option<LocalContactId>,

    #[DbField]
    pub remote_id: Option<ContactId>,

    pub cards: Vec<ContactCard>,
    pub contact_emails: Vec<ContactEmail>,

    #[DbField]
    pub create_time: u64,

    pub label_ids: Vec<ContactGroupId>,

    #[DbField]
    pub modify_time: u64,

    #[DbField]
    pub name: String,

    #[DbField]
    pub size: u64,

    #[DbField]
    pub uid: ContactUID,

    /// Reflects whether the record has been deleted. This is used to ensure that
    /// delete happens in a two-step process, where the record is marked as
    /// deleted, then deleted from remote, then finally deleted from the local
    /// by event loop update.
    #[DbField]
    pub deleted: bool,
}

impl ModelIdExtension for Contact {
    type RemoteId = ContactId;
    fn remote_id(&self) -> Option<&Self::RemoteId> {
        self.remote_id.as_ref()
    }
}

#[derive(Debug, Error)]
#[error("Cannot merge vCards: duplicate property found")]
pub struct DuplicatedVCardProperty;

impl Contact {
    pub const INIT_KEY: InitializationKey = InitializationKey::new("contacts");

    /// Returns the associated cards for a contact.
    ///
    /// This function retrieves the cards for a contact from the database,
    /// stores them in the contact struct, and then returns them.
    ///
    pub async fn cards(&mut self, tether: &Tether<UserDb>) -> Result<&[ContactCard], StashError> {
        self.cards = ContactCard::find(
            "WHERE remote_contact_id = ?",
            params![self.remote_id.clone()],
            tether,
        )
        .await?;

        Ok(&self.cards)
    }

    /// Fetches and decrypts all of the vcards for this contact.
    pub async fn vcards<T: PGPProviderSync>(
        &self,
        tether: &Tether,
        provider: &T,
        keys: &UnlockedUserKeys<T>,
    ) -> Result<Vec<VCard>, anyhow::Error> {
        let cards = ContactCard::find(
            "WHERE remote_contact_id = ?",
            params![self.remote_id.clone()],
            tether,
        )
        .await?;

        let mut decrypted_cards = vec![];

        for card in cards {
            let card = match card.decrypt_and_verify_sync(provider, keys, keys) {
                Ok(card) => card,
                Err(e) => {
                    error!("{e:?}");
                    continue;
                }
            };

            for vcard in VcardParser::new(card.reader()) {
                let vcard = vcard?.try_into()?;
                decrypted_cards.push(vcard);
            }
        }

        Ok(decrypted_cards)
    }

    pub async fn vcard_details<T: PGPProviderSync>(
        &self,
        tether: &Tether,
        provider: &T,
        keys: &UnlockedUserKeys<T>,
    ) -> anyhow::Result<VCard> {
        let cards = ContactCard::find(
            "WHERE remote_contact_id = ?",
            params![self.remote_id.clone()],
            tether,
        )
        .await?;

        let blobs = cards
            .into_iter()
            .map(|contact_card| {
                contact_card
                    .decrypt_and_verify_sync(provider, keys, keys)
                    .context("Error decrypting vCard")
            })
            .collect::<anyhow::Result<Vec<_>>>()?;

        Self::merged_vcards_from_decrypted_blobs(blobs)
    }

    /// Parses one decrypted blob (which may contain multiple vCards) into `Vec<VCard>`
    fn vcards_from_bytes(bytes: &[u8]) -> anyhow::Result<Vec<VCard>> {
        let text = String::from_utf8_lossy(bytes).into_owned();
        let mut vcards = Vec::new();
        let parser = VcardParser::new(Cursor::new(text.as_bytes()));

        for card_res in parser {
            let vcard = card_res.context("Can't parse vCard with ical")?;
            let vcard: VCard = vcard
                .try_into()
                .context("Error parsing vCard with proton-vcard")?;
            vcards.push(vcard);
        }
        Ok(vcards)
    }

    /// Merges one or more decrypted vCard blobs into a single `VCard`
    fn merged_vcards_from_decrypted_blobs(blobs: Vec<Vec<u8>>) -> anyhow::Result<VCard> {
        let mut merged: Option<VCard> = None;

        for bytes in blobs {
            let vcards = Self::vcards_from_bytes(&bytes)?;
            for vcard in vcards {
                merged = Some(match merged {
                    Some(acc) => Self::merged_disjoint(acc, vcard)?,
                    None => vcard,
                });
            }
        }

        merged.context("No VCARD data in provided blobs")
    }

    fn merged_disjoint(lhs: VCard, rhs: VCard) -> Result<VCard, DuplicatedVCardProperty> {
        fn pick<Property>(
            lhs: HashMap<PropertyUid, Property>,
            rhs: HashMap<PropertyUid, Property>,
        ) -> Result<HashMap<PropertyUid, Property>, DuplicatedVCardProperty> {
            match (lhs.is_empty(), rhs.is_empty()) {
                (true, true) => Ok(HashMap::new()),
                (true, false) => Ok(rhs),
                (false, true) => Ok(lhs),
                (false, false) => Err(DuplicatedVCardProperty),
            }
        }

        let mut merged = VCard::default();

        merged.addresses = pick(lhs.addresses, rhs.addresses)?;
        merged.calendar_addresses = pick(lhs.calendar_addresses, rhs.calendar_addresses)?;
        merged.calendar_user_addresses =
            pick(lhs.calendar_user_addresses, rhs.calendar_user_addresses)?;
        merged.categories = pick(lhs.categories, rhs.categories)?;
        merged.client_pid_map = pick(lhs.client_pid_map, rhs.client_pid_map)?;
        merged.emails = pick(lhs.emails, rhs.emails)?;
        merged.fburls = pick(lhs.fburls, rhs.fburls)?;
        merged.formatted_names = pick(lhs.formatted_names, rhs.formatted_names)?;
        merged.geos = pick(lhs.geos, rhs.geos)?;
        merged.impps = pick(lhs.impps, rhs.impps)?;
        merged.keys = pick(lhs.keys, rhs.keys)?;
        merged.languages = pick(lhs.languages, rhs.languages)?;
        merged.logos = pick(lhs.logos, rhs.logos)?;
        merged.members = pick(lhs.members, rhs.members)?;
        merged.nicknames = pick(lhs.nicknames, rhs.nicknames)?;
        merged.notes = pick(lhs.notes, rhs.notes)?;
        merged.organizations = pick(lhs.organizations, rhs.organizations)?;
        merged.photos = pick(lhs.photos, rhs.photos)?;
        merged.related = pick(lhs.related, rhs.related)?;
        merged.roles = pick(lhs.roles, rhs.roles)?;
        merged.sounds = pick(lhs.sounds, rhs.sounds)?;
        merged.sources = pick(lhs.sources, rhs.sources)?;
        merged.telephones = pick(lhs.telephones, rhs.telephones)?;
        merged.time_zones = pick(lhs.time_zones, rhs.time_zones)?;
        merged.titles = pick(lhs.titles, rhs.titles)?;
        merged.urls = pick(lhs.urls, rhs.urls)?;
        merged.xmls = pick(lhs.xmls, rhs.xmls)?;
        merged.xtendeds = pick(lhs.xtendeds, rhs.xtendeds)?;

        merged.anniversary = lhs.anniversary.or(rhs.anniversary);
        merged.birthday = lhs.birthday.or(rhs.birthday);
        merged.gender = lhs.gender.or(rhs.gender);
        merged.kind = lhs.kind.or(rhs.kind);
        merged.name = lhs.name.or(rhs.name);
        merged.product_id = lhs.product_id.or(rhs.product_id);
        merged.revision = lhs.revision.or(rhs.revision);
        merged.uid = lhs.uid.or(rhs.uid);

        Ok(merged)
    }

    /// Updates all user contacts including their emails without their cards.
    ///
    /// The result of this function MUST ONLY be used (as in [`SyncedContacts::store`]) after syncing contact labels.
    #[tracing::instrument(skip(api))]
    #[allow(clippy::too_many_lines)]
    pub async fn sync_without_contact_groups(
        api: &Session,
    ) -> Result<SyncedContacts, ApiServiceError> {
        info!("Syncing contacts without groups");

        let contacts = PaginateContacts::fetch_all(api);
        let emails = PaginateEmails::fetch_all(api);

        let (contacts, emails) = tokio::try_join!(contacts, emails)?;
        let contacts = contacts.into_iter().map(Into::into).collect();
        let emails = emails.into_iter().map(Into::into).collect();

        Ok(SyncedContacts {
            contacts,
            emails,
            contact_groups: vec![],
        })
    }

    /// Updates all user contacts including their emails without their cards and with contact groups.
    ///
    /// The result of this function MUST ONLY be used (as in [`SyncedContacts::store`]) after syncing contact labels.
    #[tracing::instrument(skip(api))]
    #[allow(clippy::too_many_lines)]
    pub async fn sync_with_contact_groups(
        api: &Session,
    ) -> Result<SyncedContacts, ApiServiceError> {
        info!("Syncing contacts with groups");

        let contacts = PaginateContacts::fetch_all(api);
        let emails = PaginateEmails::fetch_all(api);
        let contact_groups = api.get_contact_groups();

        let (contacts, emails, contact_groups) =
            tokio::try_join!(contacts, emails, contact_groups)?;
        let contacts = contacts.into_iter().map(Into::into).collect();
        let emails = emails.into_iter().map(Into::into).collect();
        let contact_groups = contact_groups.labels.into_iter().map(Into::into).collect();

        Ok(SyncedContacts {
            contacts,
            emails,
            contact_groups,
        })
    }

    /// Updates the full contact with the given ID including its emails and
    /// cards.
    /// Doesn't make an API request if the cards have already been synced.
    /// If you're using this from test code and you're modifying the mocks call
    /// `force_sync_with_card` instead.
    pub async fn sync_with_card(
        local_id: LocalContactId,
        api: &Session,
        tx: &mut impl RunTransaction<UserDb>,
    ) -> Result<(), ContactError> {
        let c: u32 = tx
            .tether()
            .query_value(
                "SELECT COUNT(*) FROM contact_cards WHERE local_contact_id = ?",
                params![local_id],
            )
            .await?;

        if c != 0 {
            debug!("Contact {local_id} is already synced, skipping fetch");
            return Ok(());
        }

        Self::force_sync_with_card(local_id, api, tx).await
    }

    pub async fn force_sync_with_card(
        local_id: LocalContactId,
        api: &Session,
        tx: &mut impl RunTransaction<UserDb>,
    ) -> Result<(), ContactError> {
        info!("Syncing full contact for contact id {local_id}");
        let remote_id = Contact::local_id_counterpart(local_id, tx.tether())
            .await?
            .ok_or(ContactError::ContactDoesNotHaveRemoteId(local_id))?;

        let mut contact_with_card = Contact::from(
            api.get_contact(remote_id.clone())
                .await
                .map_err(|err| {
                    error!("Failed to fetch full contact with id {local_id:?}: {err:?}");
                    err
                })?
                .contact,
        );

        tx.run_write_tx(async |tx| {
            contact_with_card.save(tx).await.map_err(|err| {
                error!("Failed to sync full contact to db: {err:?}");
                err
            })?;

            Ok(())
        })
        .await
        .map_err(|e| ContactError::Stash(e.into()))?;
        Ok(())
    }

    pub async fn sync_contacts_by_ids(
        api: &Session,
        contact_ids: Vec<ContactId>,
        tx: &mut impl RunTransaction<UserDb>,
    ) -> Result<Vec<Self>, ApiServiceError> {
        let batch_size = 10;
        let mut contacts: Vec<Self> = Vec::new();

        for batch in contact_ids.chunks(batch_size) {
            let requests = batch.iter().map(|id| api.get_contact(id.clone()));
            let responses = try_join_all(requests).await?;

            contacts.extend(
                responses
                    .into_iter()
                    .map(|response| response.contact.into()),
            );
        }

        tx.run_write_tx(async |tx| {
            for contact in &mut contacts {
                contact.save(tx).await?;
            }
            Ok(())
        })
        .await
        .map_err(ApiServiceError::from)?;

        Ok(contacts)
    }

    /// Returns a list of contacts grouped by the first letter of their name.
    ///
    #[tracing::instrument(skip_all)]
    pub async fn contact_list(tether: &Tether<UserDb>) -> Result<Vec<GroupedContacts>, StashError> {
        let (contacts, contact_groups) = try_join!(
            Contact::find("WHERE deleted = 0", vec![], tether),
            ContactGroup::all(tether),
        )?;

        Ok(build_grouped_contacts(contacts, contact_groups))
    }

    #[tracing::instrument(skip(tether))]
    pub async fn contact_group_by_id(
        tether: &Tether,
        id: LocalContactGroupId,
    ) -> Result<ContactGroupItem, StashError> {
        let l = ContactGroup::find_by_id(id, tether)
            .await?
            .context("The specified id doesn't exist")?;

        let mut res = ContactEmail::load_inner(
            "SELECT contact_emails.* FROM contact_emails
             JOIN contacts ON contact_emails.remote_contact_id = contacts.remote_id
             JOIN contact_email_groups AS cgs ON cgs.local_contact_email_id = contact_emails.local_id AND local_contact_group_id =?
             WHERE contacts.deleted = 0
             ORDER BY contact_emails.display_order, contact_emails.local_id",
            params![id],
            tether,
        )
        .await?;

        res.sort_unstable_by_key(|x| (x.display_order, x.id()));

        Ok(ContactGroupItem {
            local_id: l.id().into(),
            avatar_information: l.name.as_str().into(),
            name: l.name,
            contacts: res.into_iter().map(email_item_from_mail).collect(),
        })
    }

    /// Returns a list of contact suggestions (used for example in Composer). Sorted, deduplicated but not filtered by the query.
    ///
    /// # Parameters
    ///
    /// * `device_contacts` - contacts stored in the device storage, not shared between proton clients.
    ///
    pub async fn contact_suggestions(
        device_contacts: Vec<DeviceContact>,
        tether: &Tether,
    ) -> Result<ContactSuggestions, StashError> {
        let (contacts, contact_groups) = try_join!(
            Contact::find("WHERE deleted = 0", vec![], tether),
            ContactGroup::all(tether),
        )?;

        Ok(ContactSuggestions::from_contacts_and_device_contacts(
            contacts,
            contact_groups,
            device_contacts,
        ))
    }

    /// Marks a contact as deleted.
    /// Deletion is two-step process: first, the record is marked as deleted in
    /// the database, then it is deleted from the remote server, and finally
    /// It is deleted from the local database by the event loop update.
    ///
    pub async fn mark_delete(&mut self, bond: &WriteTx<'_>) -> Result<(), StashError> {
        self.deleted = true;
        self.save(bond).await
    }

    /// Marks a contact as undeleted.
    /// This method serves as the reverse of [`Contact::mark_delete()`].
    /// which can revert the deletion of a contact in case of something unpredictable happened.
    ///
    pub async fn mark_undelete(&mut self, bond: &WriteTx<'_>) -> Result<(), StashError> {
        self.deleted = false;
        self.save(bond).await
    }

    pub async fn delete_from_remote(
        remote_ids: &[ContactId],
        api: &Session,
    ) -> Result<Vec<ContactId>, ApiServiceError> {
        use mail_core_api::consts::General;
        info!("Deleting contacts {remote_ids:?}");
        let response = api
            .put_delete_contacts(remote_ids.iter().cloned().map_into().collect())
            .await?;

        Ok(response
            .responses
            .iter()
            .filter(|r| r.response.code != General::NoError as u32)
            .map(|r| r.id.clone())
            .collect())
    }

    pub async fn watch_contact_list(
        mail_stash: &Stash<UserDb>,
    ) -> Result<(Vec<GroupedContacts>, WatcherHandle), StashError> {
        let tether = mail_stash.connection();
        let contacts = Contact::contact_list(&tether).await?;
        let handle = mail_stash
            .subscribe_to(|sender| Box::new(ContactListWatcher { sender }))
            .await?;
        Ok((contacts, handle))
    }

    pub async fn handle_event(
        tx: &WriteTx<'_>,
        id: &ContactId,
        action: mail_shared_types::Action,
        contact: Option<&mut Contact>,
        changeset: &mut RebaseChangeSet,
    ) -> Result<(), StashError> {
        use mail_shared_types::Action;

        action
            .log_entry(id, async |remote_id| {
                Contact::remote_id_counterpart(remote_id.clone(), tx)
                    .await
                    .unwrap_or_default()
                    .map(|v| v.as_u64())
            })
            .await;

        match action {
            Action::Delete => tx
                .execute(
                    "DELETE FROM contacts WHERE remote_id = ?",
                    params![id.clone()],
                )
                .await
                .map(|_| ())
                .map_err(|e| {
                    error!("Failed to delete contact: {e:?}");
                    e
                })?,
            Action::Create | Action::Update => {
                if let Some(contact) = contact {
                    contact.save(tx).await.map_err(|e| {
                        error!("Failed to create or update contact: {e:?}");
                        e
                    })?;
                    changeset.add(contact.id());
                }
            }
            Action::UpdateFlags => (),
        }
        Ok(())
    }

    pub async fn without_emails(
        tether: &Tether<UserDb>,
    ) -> Result<Vec<LocalContactId>, StashError> {
        tether.query_values::<_, LocalContactId>(indoc! {
            "SELECT contacts.local_id FROM contacts WHERE contacts.remote_id NOT IN (SELECT DISTINCT remote_contact_id FROM contact_emails)"
        }, params![]).await
    }
}

impl ModelHooks for Contact {
    fn before_save(&mut self, tx: &Transaction<'_>) -> StashResult<()> {
        // WARN: For performance reasons this will NOT be called in the initial sync. See `SyncedContacts::store`
        // Any extra logic here should be copied there.
        if let Some(remote_id) = &self.remote_id {
            if let Some(existing) = Self::find_by_remote_id_sync(remote_id, tx)? {
                self.local_id = existing.local_id;
            }
        } else if let Some(local_id) = self.local_id
            && let Some(existing) = Self::load_by_id_sync(local_id, tx)?
        {
            self.remote_id = existing.remote_id;
        }

        Ok(())
    }

    fn after_save(&mut self, tx: &Transaction<'_>) -> Result<(), StashError> {
        // handle contact groups
        tx.execute(
            "DELETE FROM contact_contact_groups WHERE local_contact_id = ?",
            mail_stash::rusqlite::params![self.id()],
        )?;

        if !self.label_ids.is_empty() {
            ContactGroup::link_contact_groups_for_contact(tx, self.id(), &self.label_ids)?;
        }

        for card in &mut self.cards {
            card.local_contact_id = self.local_id;
            card.remote_contact_id.clone_from(&self.remote_id);
        }

        for email in &mut self.contact_emails {
            email.local_contact_id = self.local_id;
            email.remote_contact_id.clone_from(&self.remote_id);
            email
                .save_sync(tx)
                .inspect_err(|e| error!("Failed to save contact {:?}: {e}", email.remote_id))?;
        }

        let local_email_contact_ids = self
            .contact_emails
            .iter()
            .map(|e| e.local_id.expect("Should be set"))
            .collect::<Vec<_>>();

        let mut query = tx.prepare(&format!(
            "DELETE FROM contact_emails WHERE local_id NOT IN ({placeholders}) AND local_contact_id=?",
            placeholders = placeholders(&local_email_contact_ids)
        ))?;

        query
            .execute(params_from_iter(
                local_email_contact_ids
                    .iter()
                    .map(LocalContactEmailId::as_u64)
                    .chain(std::iter::once(
                        self.local_id.expect("Should be set").as_u64(),
                    )),
            ))
            .inspect_err(|e| error!("Failed to delete contact emails: {e}"))?;

        tx.execute(
            "DELETE FROM contact_cards WHERE local_contact_id = ?",
            (self.local_id,),
        )?;
        for card in &mut self.cards {
            card.local_id = None;
            card.save_sync(tx).map_err(|e| {
                error!("Failed to update contact cards: {e:?}");
                e
            })?;
        }
        Ok(())
    }

    fn after_load(&mut self, conn: &Connection) -> StashResult<()> {
        let label_ids: Vec<ContactGroupId> = conn.query_rows_col(
            indoc! {
                "SELECT remote_id FROM contact_group WHERE local_id IN (
                SELECT local_contact_group_id FROM contact_contact_groups WHERE local_contact_id = ?
            ) AND remote_id IS NOT NULL"
            },
            rusqlite::params![self.id()],
        )?;

        self.contact_emails = ContactEmail::find_sync(
            "WHERE local_contact_id = ? ORDER BY display_order ASC",
            [self.local_id.expect("Should be set")],
            conn,
        )?;

        self.label_ids = label_ids;
        Ok(())
    }
}

impl From<ApiContactBasic> for Contact {
    fn from(value: ApiContactBasic) -> Self {
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

impl From<ApiContactFull> for Contact {
    fn from(value: ApiContactFull) -> Self {
        Self {
            local_id: None,
            remote_id: Some(value.id),
            cards: value.cards.map_vec(),
            contact_emails: value
                .contact_emails
                .into_iter()
                .map(ContactEmail::from)
                .collect(),
            create_time: value.create_time,
            label_ids: value.label_ids.map_vec(),
            modify_time: value.modify_time,
            name: value.name,
            size: value.size,
            uid: value.uid,
            deleted: false,
        }
    }
}

impl Contact {
    #[cfg(feature = "test-utils")]
    #[allow(clippy::default_trait_access)]
    #[must_use]
    pub fn test_default() -> Self {
        Self {
            local_id: Default::default(),
            remote_id: Default::default(),
            cards: Default::default(),
            contact_emails: Default::default(),
            create_time: Default::default(),
            label_ids: Default::default(),
            modify_time: Default::default(),
            name: Default::default(),
            size: Default::default(),
            uid: ContactUID::from(String::default()),
            deleted: Default::default(),
        }
    }
}

pub struct ContactListWatcher {
    sender: flume::Sender<()>,
}

impl ContactListWatcher {
    #[must_use]
    pub fn new(sender: flume::Sender<()>) -> Self {
        Self { sender }
    }
}

impl TableObserver for ContactListWatcher {
    fn tables(&self) -> Vec<String> {
        vec![
            Contact::table_name().to_string(),
            ContactEmail::table_name().to_string(),
        ]
    }

    fn on_tables_changed(&self, _changed_tables: &BTreeSet<String>) {
        self.sender
            .send(())
            .inspect_err(|e| error!("Failed to send notification for ContactListWatcher: {e:?}"))
            .ok();
    }
}

#[must_use]
#[derive(Debug)]
pub struct SyncedContacts {
    contact_groups: Vec<ContactGroup>,
    contacts: Vec<Contact>,
    emails: Vec<ContactEmail>,
}

impl SyncedContacts {
    #[tracing::instrument(skip_all)]
    pub fn store(self, tx: &Transaction<'_>) -> Result<(), StashError> {
        let Self {
            contacts,
            mut emails,
            mut contact_groups,
        } = self;

        if !contact_groups.is_empty() {
            let tgroups = Instant::now();
            for contact_group in &mut contact_groups {
                contact_group.save_sync(tx)?;
            }
            debug!(
                "Stored {} contact groups to the db in {:?}",
                contact_groups.len(),
                tgroups.elapsed()
            );
        }

        tx.execute_batch(
            "
        DELETE FROM contacts;
        DELETE FROM contact_emails;
        DELETE FROM contact_cards;
        DELETE FROM contact_email_labels;",
        )?;

        let mut id_map = HashMap::new();

        let t1 = Instant::now();
        let mut q = tx.prepare(Contact::INSERT_QUERY)?;
        let mut contact_groups_stmt = tx.prepare(LINK_CONTACT_GROUPS_CONTATCS_QUERY)?;
        for cont in contacts {
            let params = params_from_iter(cont.field_values());
            let id = q.query_row(params, |r| r.get(0))?;
            id_map.insert(cont.remote_id.clone().unwrap(), id);

            for cg_id in cont.label_ids {
                contact_groups_stmt.execute(rusqlite::params![id, cg_id])?;
            }
        }
        drop(contact_groups_stmt);
        debug!(
            "Stored {} contacts to the db in {:?}",
            id_map.len(),
            t1.elapsed()
        );

        emails.retain_mut(|em| {
            let Some(contact_id) = &em.remote_contact_id else {
                error!("a contact_email has no contact");
                return false;
            };
            let Some(local_id) = id_map.get(contact_id) else {
                error!("a contact_email has no saved local contact");
                return false;
            };
            em.local_contact_id = Some(*local_id);
            true
        });

        let t2 = Instant::now();
        let mut q = tx.prepare(ContactEmail::INSERT_QUERY)?;
        let count = emails.len();
        let mut contact_email_groups_stmt = tx.prepare(LINK_CONTACT_GROUPS_EMAILS_QUERY)?;
        for em in emails {
            let params = params_from_iter(em.field_values());
            let contact_email_id: LocalContactEmailId = q.query_row(params, |r| r.get(0))?;

            for id in em.label_ids {
                contact_email_groups_stmt.execute(rusqlite::params![contact_email_id, id])?;
            }
        }

        debug!(
            "Stored {count} contacts_emails to the db in {:?}",
            t2.elapsed()
        );

        debug!("Stored all to the db in {:?}", t1.elapsed());
        Ok(())
    }
}

struct PaginateContacts;
impl Paginatable for PaginateContacts {
    type PaginateOptions = GetContactsOptions;
    type Response = GetContactsResponse;
    type Output = ApiContactBasic;
    type Error = ApiServiceError;
    type API = Session;
    const NAME: &'static str = "Contacts";
    const DEFAULT_PAGE_SIZE: u64 = SYNC_CONTACT_PAGE_SIZE;

    async fn fetch(
        api: &Self::API,
        options: Self::PaginateOptions,
    ) -> Result<Self::Response, Self::Error> {
        api.get_contacts(options).await
    }
}

struct PaginateEmails;
impl Paginatable for PaginateEmails {
    type PaginateOptions = GetContactsEmailsOptions;
    type Response = GetContactsEmailsResponse;
    type Output = ApiContactEmail;
    type Error = ApiServiceError;
    type API = Session;
    const NAME: &'static str = "Emails";
    const DEFAULT_PAGE_SIZE: u64 = SYNC_CONTACT_PAGE_SIZE;

    async fn fetch(
        api: &Self::API,
        options: Self::PaginateOptions,
    ) -> Result<Self::Response, Self::Error> {
        api.get_contacts_emails(options).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contact_card::ContactCard;
    use crate::contact_email::ContactEmail;
    use crate::test_utils::new_contact_test_connection;
    use crate::types::{ContactSendingPreferences, ContactTypes};
    use mail_core_api::services::proton::{ContactEmailId, ContactId, ContactUID};
    use mail_stash::orm::Model;
    use mail_stash::params;
    use mail_stash::stash::StashError;
    use proton_crypto_account::contacts::ContactCardType;

    #[tokio::test]
    async fn test_full_contact() {
        let mut tether = new_contact_test_connection().await.connection();
        // crate contact grouped so it can be resolved.
        tether
            .write_tx(async |tx| {
                ContactGroup {
                    remote_id: Some("I6hgx3Ol-d3HYa3E394T_ACXDmTaBub14w==".into()),
                    ..ContactGroup::test_default()
                }
                .save(tx)
                .await
            })
            .await
            .unwrap();
        let mut full_contact = create_test_full_contact();
        let local_id = tether
            .write_tx::<_, _, StashError>(async |tx| {
                full_contact
                    .save(tx)
                    .await
                    .expect("failed to create contact");
                let id = full_contact.local_id.expect("failed to get contact id");
                let local_id = full_contact.id();
                full_contact
                    .save(tx)
                    .await
                    .expect("failed to overwrite contact");
                let id_second = full_contact.local_id.expect("failed to get contact id");
                assert_eq!(id, 1.into());
                assert_eq!(id, id_second);

                Ok(local_id)
            })
            .await
            .unwrap();

        let mut contact_with_cards = Contact::load(local_id, &tether)
            .await
            .expect("query contact with cards failed")
            .expect("expected to find contact");
        let cards = contact_with_cards
            .cards(&tether)
            .await
            .expect("Failed to query cards");
        assert_eq!(cards.len(), full_contact.cards.len());
    }

    #[tokio::test]
    async fn test_partial_contact() {
        let mut tether = new_contact_test_connection().await.connection();
        let mut partial_contacts = create_test_partial_contacts();
        let mut contact_emails = create_test_contact_emails();
        tether
            .write_tx(async |tx| {
                ContactGroup {
                    remote_id: Some("I6hgx3Ol-d3HYa3E394T_ACXDmTaBub14w==".into()),
                    ..ContactGroup::test_default()
                }
                .save(tx)
                .await
            })
            .await
            .unwrap();
        tether
            .write_tx::<_, _, StashError>(async |tx| {
                for contact in &mut partial_contacts {
                    contact.save(tx).await.expect("failed to create contact");
                }
                for contact_email in &mut contact_emails {
                    contact_email.remote_contact_id =
                        partial_contacts.first().unwrap().remote_id.clone();
                    contact_email
                        .save(tx)
                        .await
                        .expect("failed to create contact email");
                }
                Ok(())
            })
            .await
            .unwrap();

        assert_eq!(
            partial_contacts.first().unwrap().local_id.unwrap(),
            1.into()
        );
        assert_eq!(contact_emails.first().unwrap().local_id.unwrap(), 1.into());

        let mail = ContactEmail::find_first(
            "WHERE canonical_email = ?",
            params!["contact_email_1@contact.test"],
            &tether,
        )
        .await
        .expect("failed to query email")
        .expect("expected to find contact email");
        assert_eq!(
            mail.canonical_email.as_clear_text_str(),
            "contact_email_1@contact.test"
        );

        let mails = ContactEmail::find("LIMIT 100", vec![], &tether)
            .await
            .expect("failed to query email");
        assert_eq!(mails.len(), contact_emails.len());

        let mut contacts = Contact::find("LIMIT 100", vec![], &tether)
            .await
            .expect("failed to query contacts");
        let contact = contacts.first_mut().unwrap();
        assert_eq!(
            contact.remote_id,
            Some(ContactId::from("a29olIjFv0rnXxBhSMw=="))
        );
        assert_eq!(contact.contact_emails.len(), contact_emails.len());

        let mut contact_single = Contact::load(contact.id(), &tether)
            .await
            .expect("failed to query contacts")
            .expect("expected to find contact");
        contact_single
            .cards(&tether)
            .await
            .expect("failed to query cards");
        assert_eq!(&contact_single, contact);
    }

    fn create_test_full_contact() -> Contact {
        Contact {
            local_id: None,
            remote_id: Some(ContactId::from("a29olIjFv0rnXxBhSMw==")),
            name: "contact_name".to_owned(),
            uid: ContactUID::from("proton-legacy-139892c2-f691-4118-8c29-061196013e04"),
            size: 1443,
            create_time: 1_503_815_366,
            modify_time: 1_503_815_366,
            contact_emails: create_test_contact_emails(),
            label_ids: vec![ContactGroupId::from("I6hgx3Ol-d3HYa3E394T_ACXDmTaBub14w==")],
            deleted: false,
            cards: vec![
                ContactCard {
                    local_id: None,
                    local_contact_id: None,
                    remote_contact_id: Some(ContactId::from("a29olIjFv0rnXxBhSMw==")),
                    card_type: ContactCardType::Signed,
                    data: r"    BEGIN:VCARD\n    VERSION:4.0\n    FN:ProtonMail Features\n    UID:proton-legacy-139892c2-f691-4118-8c29-061196013e04\n    item1.EMAIL;TYPE=work;PREF=1:features@protonmail.black\n    item2.EMAIL;TYPE=home;PREF=2:features@protonmail.ch\n    END:VCARD".to_owned(),
                    signature: Some("-----BEGIN PGP SIGNATURE-----.*-----END PGP SIGNATURE-----".to_owned()),
                },
                ContactCard {
                    local_id: None,
                    local_contact_id: None,
                    remote_contact_id: Some(ContactId::from("a29olIjFv0rnXxBhSMw==")),
                    card_type: ContactCardType::EncryptedAndSigned,
                    data: "-----BEGIN PGP MESSAGE-----.*-----END PGP MESSAGE-----".to_owned(),
                    signature: Some("-----BEGIN PGP SIGNATURE-----.*-----END PGP SIGNATURE-----".to_owned()),
                },
            ],
        }
    }

    fn create_test_contact_emails() -> Vec<ContactEmail> {
        vec![
            ContactEmail {
                local_id: None,
                remote_id: Some(ContactEmailId::from("aefew4323jFv0BhSMw==")),
                name: "contact_email_name_1".to_owned(),
                email: "contact_email_1@contact.test".into(),
                contact_type: ContactTypes::new(vec!["work".to_owned()]),
                defaults: ContactSendingPreferences::Default,
                display_order: 1,
                remote_contact_id: Some(ContactId::from("a29olIjFv0rnXxBhSMw==")),
                local_contact_id: None,
                label_ids: vec![ContactGroupId::from("I6hgx3Ol-d3HYa3E394T_ACXDmTaBub14w==")],
                canonical_email: "contact_email_1@contact.test".into(),
                last_used_time: 0.into(),
                is_proton: true,
            },
            ContactEmail {
                local_id: None,
                remote_id: Some(ContactEmailId::from("aefew4323jFv0BhSMz==")),
                name: "contact_email_name_2".to_owned(),
                email: "contact_email_2@contact.test".into(),
                contact_type: ContactTypes::new(vec!["work".to_owned()]),
                defaults: ContactSendingPreferences::Default,
                display_order: 1,
                remote_contact_id: Some(ContactId::from("a29olIjFv0rnXxBhSMw==")),
                local_contact_id: None,
                label_ids: vec![ContactGroupId::from("I6hgx3Ol-d3HYa3E394T_ACXDmTaBub14w==")],
                canonical_email: "contact_email_2@contact.test".into(),
                last_used_time: 0.into(),
                is_proton: true,
            },
        ]
    }

    fn create_test_partial_contacts() -> Vec<Contact> {
        vec![Contact {
            local_id: None,
            remote_id: Some(ContactId::from("a29olIjFv0rnXxBhSMw==")),
            name: "contact_name".to_owned(),
            uid: ContactUID::from("proton-legacy-139892c2-f691-4118-8c29-061196013e04".to_owned()),
            size: 1443,
            create_time: 1_503_815_366,
            modify_time: 1_503_815_366,
            contact_emails: vec![],
            label_ids: vec![ContactGroupId::from("I6hgx3Ol-d3HYa3E394T_ACXDmTaBub14w==")],
            cards: vec![],
            deleted: false,
        }]
    }

    mod contact_watcher {
        use mail_stash::orm::Model;
        use mail_stash::params;

        use crate::contact::Contact;
        use crate::test_utils::new_contact_test_connection;

        #[tokio::test]
        async fn test_contact_list_watcher() {
            let mail_stash = new_contact_test_connection().await;
            let mut tether = mail_stash.connection();
            let mut contact =
                crate::contact!(remote_id: crate::cid!("123"), name: "Barbara Fox".to_string());
            tether
                .write_tx(async |tx| contact.save(tx).await)
                .await
                .unwrap();
            let (_, list_receiver) = Contact::watch_contact_list(&mail_stash).await.unwrap();
            let list_receiver = list_receiver.receiver;

            tether
                .write_tx(async |tx| {
                    contact.name = "Barbara Lox".to_string();
                    contact.save(tx).await
                })
                .await
                .unwrap();

            assert!(list_receiver.recv_async().await.is_ok());

            tether
                .write_tx(async |tx| {
                    contact.deleted = true;
                    contact.save(tx).await
                })
                .await
                .unwrap();

            assert!(list_receiver.recv_async().await.is_ok());

            tether
                .write_tx(async |tx| {
                    contact.deleted = false;
                    contact.save(tx).await
                })
                .await
                .unwrap();

            assert!(list_receiver.recv_async().await.is_ok());

            tether
                .write_tx(async |tx| {
                    tx.execute(
                        "DELETE FROM contacts WHERE local_id = ?",
                        params![contact.local_id],
                    )
                    .await
                })
                .await
                .unwrap();
            let all_contacts = Contact::find("", vec![], &tether).await.unwrap();
            assert_eq!(all_contacts.len(), 0);

            assert!(list_receiver.recv_async().await.is_ok());
        }
    }
}
