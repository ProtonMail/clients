use lattice::{LtApiCode, LtApiResponse};
use mail_contacts_api::GetContactEventLatestRequest;
use mail_core_api::services::proton::{EventId, GetEventsLatestResponse};
use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

use super::test_context::TestContext;

impl TestContext {
    pub async fn mock_last_event_id(&self, id: EventId) {
        Mock::given(method("GET"))
            .and(path("/api/core/v4/events/latest"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(GetEventsLatestResponse { event_id: id }),
            )
            .expect(1) // this should only ever be initialized once at the moment
            .named("Setup user get latest events")
            .mount(self.mock_server())
            .await;
    }

    pub async fn mock_last_event_id_core_v6(&self, id: EventId) {
        Mock::given(method("GET"))
            .and(path("/api/core/v6/events/latest"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(GetEventsLatestResponse { event_id: id }),
            )
            .expect(1) // this should only ever be initialized once at the moment
            .named("Setup user get latest events")
            .mount(self.mock_server())
            .await;
    }

    pub async fn mock_last_event_id_contacts_v6(&self, id: EventId) {
        GetContactEventLatestRequest::mock()
            .respond_with(ResponseTemplate::new(200).set_body_json(LtApiResponse {
                code: LtApiCode::OK,
                body: GetEventsLatestResponse { event_id: id },
            }))
            .expect(1) // this should only ever be initialized once at the moment
            .named("Setup user get latest events")
            .mount(self.mock_server())
            .await;
    }
}
