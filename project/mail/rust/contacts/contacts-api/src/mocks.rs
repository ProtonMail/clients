use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, MockBuilder, MockServer, ResponseTemplate, Times};

use crate::{
    ContactBasic, ContactEmail, ContactFull, ContactId, GetContactResponse,
    GetContactsEmailsResponse, GetContactsResponse, PutDeleteContactResponse, PutDeleteContacts,
    PutDeleteContactsResponse,
};
use mail_api_shared::ApiErrorInfo;

/// Proton API error code for a resource that does not exist.
const NOT_EXISTS_CODE: u32 = 2501;

/// Extension trait that mounts contact-related wiremock stubs onto a [`MockServer`].
#[allow(async_fn_in_trait)]
pub trait ContactsMockServerExt {
    /// Mock `GET /contacts/v4/contacts` returning the given contacts, expected exactly once.
    async fn mock_get_all_contacts_partial_request(&self, contacts: Vec<ContactBasic>);

    /// Mock `GET /contacts/v4/contacts` returning the given contacts the specified number of times.
    async fn mock_get_contacts(
        &self,
        contacts: Option<Vec<ContactBasic>>,
        expect: impl Into<Times>,
    );

    /// Mount a fully custom mock for `GET /contacts/v4/contacts`.
    ///
    /// The closure receives a pre-built [`MockBuilder`] already scoped to the
    /// contacts list endpoint; add further matchers or a response template and
    /// return the finished [`Mock`].
    async fn mock_get_contacts_respond_with(&self, respond_with: impl Fn(MockBuilder) -> Mock);

    /// Mock `GET /contacts/v4/contacts/emails` returning the given emails the specified number of times.
    async fn mock_get_contacts_emails(
        &self,
        contact_emails: Option<Vec<ContactEmail>>,
        expect: impl Into<Times>,
    );

    /// Mock `GET /contacts/v4/contacts/emails` returning the given emails, expected exactly once.
    async fn mock_get_all_contact_emails_request(&self, contact_emails: Vec<ContactEmail>);

    /// Mock `GET /contacts/v4/contacts/{id}` returning the given full contact.
    async fn mock_get_full_contact(&self, contact: ContactFull);

    /// Mock `GET /contacts/v4/contacts/{id}` returning a 422 not-exists error.
    async fn mock_get_full_contact_does_not_exist(&self, contact_id: ContactId);

    /// Mock `PUT /contacts/v4/contacts/delete` for the given contact ids, expected exactly once.
    async fn mock_delete_contacts(&self, contact_ids: Vec<ContactId>);
}

impl ContactsMockServerExt for MockServer {
    async fn mock_get_all_contacts_partial_request(&self, contacts: Vec<ContactBasic>) {
        let num_contacts = contacts.len() as u64;
        Mock::given(method("GET"))
            .and(path("/api/contacts/v4/contacts"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(GetContactsResponse {
                    contacts,
                    total: num_contacts,
                }),
            )
            .expect(1)
            .named("mock_get_all_contacts_partial_request")
            .mount(self)
            .await;
    }

    async fn mock_get_contacts(
        &self,
        contacts: Option<Vec<ContactBasic>>,
        expect: impl Into<Times>,
    ) {
        let contacts = contacts.unwrap_or_default();
        Mock::given(method("GET"))
            .and(path("/api/contacts/v4/contacts"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(GetContactsResponse {
                    total: contacts.len() as u64,
                    contacts,
                }),
            )
            .expect(expect)
            .named("mock_get_contacts")
            .mount(self)
            .await;
    }

    async fn mock_get_contacts_respond_with(&self, respond_with: impl Fn(MockBuilder) -> Mock) {
        let mock = Mock::given(method("GET")).and(path("/api/contacts/v4/contacts"));
        respond_with(mock)
            .named("mock_get_contacts_respond_with")
            .mount(self)
            .await;
    }

    async fn mock_get_contacts_emails(
        &self,
        contact_emails: Option<Vec<ContactEmail>>,
        expect: impl Into<Times>,
    ) {
        let contact_emails = contact_emails.unwrap_or_default();
        Mock::given(method("GET"))
            .and(path("/api/contacts/v4/contacts/emails"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(GetContactsEmailsResponse {
                    total: contact_emails.len() as u64,
                    contact_emails,
                }),
            )
            .expect(expect)
            .named("mock_get_contacts_emails")
            .mount(self)
            .await;
    }

    async fn mock_get_all_contact_emails_request(&self, contact_emails: Vec<ContactEmail>) {
        let num_contacts = contact_emails.len() as u64;
        Mock::given(method("GET"))
            .and(path("/api/contacts/v4/contacts/emails"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(GetContactsEmailsResponse {
                    contact_emails,
                    total: num_contacts,
                }),
            )
            .expect(1)
            .named("mock_get_all_contact_emails_request")
            .mount(self)
            .await;
    }

    async fn mock_get_full_contact(&self, contact: ContactFull) {
        Mock::given(method("GET"))
            .and(path(format!("/api/contacts/v4/contacts/{}", &contact.id)))
            .respond_with(ResponseTemplate::new(200).set_body_json(GetContactResponse { contact }))
            .named("mock_get_full_contact")
            .mount(self)
            .await;
    }

    async fn mock_get_full_contact_does_not_exist(&self, contact_id: ContactId) {
        Mock::given(method("GET"))
            .and(path(format!("/api/contacts/v4/contacts/{contact_id}")))
            .respond_with(ResponseTemplate::new(422).set_body_json(ApiErrorInfo {
                code: NOT_EXISTS_CODE,
                error: None,
                details: None,
            }))
            .named("mock_get_full_contact_does_not_exist")
            .mount(self)
            .await;
    }

    async fn mock_delete_contacts(&self, contact_ids: Vec<ContactId>) {
        Mock::given(method("PUT"))
            .and(path("/api/contacts/v4/contacts/delete"))
            .and(body_json(PutDeleteContacts {
                ids: contact_ids.clone(),
            }))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(PutDeleteContactsResponse {
                    responses: contact_ids
                        .into_iter()
                        .map(|id| PutDeleteContactResponse {
                            id,
                            response: ApiErrorInfo {
                                code: 1000,
                                ..Default::default()
                            },
                        })
                        .collect(),
                }),
            )
            .expect(1)
            .named("mock_delete_contacts")
            .mount(self)
            .await;
    }
}
