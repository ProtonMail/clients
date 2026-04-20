use crate::test_utils::test_context::TestContext;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, ResponseTemplate};

impl TestContext {
    #[function_name::named]
    pub async fn mock_get_images_logo(&self, address: &str, response: Vec<u8>) {
        Mock::given(method("GET"))
            .and(path("/api/core/v4/images/logo"))
            .and(query_param("Address", address))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(response))
            .expect(1)
            .named(function_name!())
            .mount(self.mock_server())
            .await;
    }
}
