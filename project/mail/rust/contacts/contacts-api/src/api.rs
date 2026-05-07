//! Contact API trait.

use crate::{
    ContactId, GetContactResponse, GetContactsEmailsOptions, GetContactsEmailsResponse,
    GetContactsOptions, GetContactsResponse, PutDeleteContactsRequest, PutDeleteContactsResponse,
};
use contact_lattice::{GetContactEvent, GetContactEventLatestRequest};
use mail_api_event_types::{EventId, GetEventsLatestResponse};
use mail_api_lattice::RunLatticeContractExt;
use mail_api_shared::ApiServiceResult;
use mail_muon::common::Sender;
use mail_muon::{ProtonRequest, ProtonResponse};

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

    fn get_contact_event_v6(
        &self,
        event_id: EventId,
    ) -> impl Future<Output = ApiServiceResult<String>> + Send;

    fn get_contact_event_latest_v6(
        &self,
    ) -> impl Future<Output = ApiServiceResult<GetEventsLatestResponse>> + Send;
}

impl<This: ?Sized + Sender<ProtonRequest, ProtonResponse>> ContactApi for This {
    async fn get_contact(&self, contact_id: ContactId) -> ApiServiceResult<GetContactResponse> {
        let resp = self
            .run_lattice_contract_compat(contact_lattice::GetContactRequest { id: contact_id })
            .await?;
        Ok(resp.0)
    }

    async fn get_contacts(
        &self,
        options: GetContactsOptions,
    ) -> ApiServiceResult<GetContactsResponse> {
        let resp = self
            .run_lattice_contract_compat(contact_lattice::GetContactsRequest { options })
            .await?;
        Ok(resp.0)
    }

    async fn get_contacts_emails(
        &self,
        options: GetContactsEmailsOptions,
    ) -> ApiServiceResult<GetContactsEmailsResponse> {
        let resp = self
            .run_lattice_contract_compat(contact_lattice::GetContactsEmailsRequest { options })
            .await?;
        Ok(resp.0)
    }

    async fn put_delete_contacts(
        &self,
        ids: Vec<ContactId>,
    ) -> ApiServiceResult<PutDeleteContactsResponse> {
        let resp = self
            .run_lattice_contract_compat(PutDeleteContactsRequest { ids })
            .await?;
        Ok(resp.0)
    }

    async fn get_contact_event_v6(&self, event_id: EventId) -> ApiServiceResult<String> {
        let resp = self
            .run_lattice_contract_compat(GetContactEvent { id: event_id })
            .await?;
        Ok(String::from_utf8(resp.0)?)
    }

    async fn get_contact_event_latest_v6(&self) -> ApiServiceResult<GetEventsLatestResponse> {
        let resp = self
            .run_lattice_contract_compat(GetContactEventLatestRequest)
            .await?;
        Ok(resp.0)
    }
}
