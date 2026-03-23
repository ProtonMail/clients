//! Contact API trait.

use crate::{
    ContactId, GetContactResponse, GetContactsEmailsOptions, GetContactsEmailsResponse,
    GetContactsOptions, GetContactsResponse, PutDeleteContacts, PutDeleteContactsResponse,
};
use mail_api_event_types::{EventId, GetEventsLatestResponse};
use mail_api_shared::ApiServiceResult;
use mail_muon::common::Sender;
use mail_muon::http::HttpReqExt;
use mail_muon::{GET, PUT};
use mail_muon::{ProtonRequest, ProtonResponse, serde_to_query};

const CONTACTS_V4: &str = "/contacts/v4/contacts";
const CONTACTS_V6: &str = "/contacts/v6";

#[allow(async_fn_in_trait)]
pub trait ContactApi {
    /// GETs a single contact.
    ///
    /// This returns the full contact record.
    async fn get_contact(&self, contact_id: ContactId) -> ApiServiceResult<GetContactResponse>;

    /// GETs a list of contacts.
    ///
    /// This returns basic information — not the full contact record.
    async fn get_contacts(
        &self,
        options: GetContactsOptions,
    ) -> ApiServiceResult<GetContactsResponse>;

    /// GETs a list of emails for contacts.
    ///
    /// This returns basic information — not the full contact record.
    async fn get_contacts_emails(
        &self,
        options: GetContactsEmailsOptions,
    ) -> ApiServiceResult<GetContactsEmailsResponse>;

    async fn put_delete_contacts(
        &self,
        ids: Vec<ContactId>,
    ) -> ApiServiceResult<PutDeleteContactsResponse>;

    async fn get_contact_event_v6(&self, event_id: EventId) -> ApiServiceResult<String>;

    async fn get_contact_event_latest_v6(&self) -> ApiServiceResult<GetEventsLatestResponse>;
}

impl<This: ?Sized + Sender<ProtonRequest, ProtonResponse>> ContactApi for This {
    async fn get_contact(&self, contact_id: ContactId) -> ApiServiceResult<GetContactResponse> {
        Ok(GET!("{CONTACTS_V4}/{contact_id}")
            .send_with(self)
            .await?
            .ok()?
            .into_body_json()?)
    }

    async fn get_contacts(
        &self,
        options: GetContactsOptions,
    ) -> ApiServiceResult<GetContactsResponse> {
        Ok(GET!("{CONTACTS_V4}")
            .query(serde_to_query(options)?)
            .send_with(self)
            .await?
            .ok()?
            .into_body_json()?)
    }

    async fn get_contacts_emails(
        &self,
        options: GetContactsEmailsOptions,
    ) -> ApiServiceResult<GetContactsEmailsResponse> {
        Ok(GET!("{CONTACTS_V4}/emails")
            .query(serde_to_query(options)?)
            .send_with(self)
            .await?
            .ok()?
            .into_body_json()?)
    }

    async fn put_delete_contacts(
        &self,
        ids: Vec<ContactId>,
    ) -> ApiServiceResult<PutDeleteContactsResponse> {
        Ok(PUT!("{CONTACTS_V4}/delete")
            .body_json(PutDeleteContacts { ids })?
            .send_with(self)
            .await?
            .ok()?
            .into_body_json()?)
    }

    async fn get_contact_event_v6(&self, event_id: EventId) -> ApiServiceResult<String> {
        Ok(GET!("{CONTACTS_V6}/events/{event_id}")
            .send_with(self)
            .await?
            .ok()?
            .into_body_string()?)
    }

    async fn get_contact_event_latest_v6(&self) -> ApiServiceResult<GetEventsLatestResponse> {
        Ok(GET!("{CONTACTS_V6}/events/latest")
            .send_with(self)
            .await?
            .ok()?
            .into_body_json()?)
    }
}
