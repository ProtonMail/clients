use crate::session::{AppVersion, Endpoint, Env, EnvId, Server, Session};
use wiremock::matchers::{method, path_regex};
use wiremock::{Mock, MockServer, ResponseTemplate};

struct MockApiEnv {
    host: Endpoint,
}

impl Env for MockApiEnv {
    fn servers(&self, _: &AppVersion) -> Vec<Server> {
        vec![Server::new(self.host.clone(), "/api")]
    }
}

/// Build a [`Session`] pointed at the given mock server.
///
/// Mounts the auth/session and token-refresh stubs that muon requires on
/// startup so that any subsequent API calls can proceed.
pub async fn test_session(mock_server: &MockServer) -> Session {
    // Muon auto-creates an auth session and refreshes tokens on startup.
    Mock::given(method("POST"))
        .and(path_regex(r".*/auth/v4/sessions$"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "ServerProof": "dummy", "UID": "dummy", "AccessToken": "dummy",
            "RefreshToken": "dummy", "Scopes": ["dummy"],
            "2FA": { "Enabled": 0 }, "PasswordMode": 1,
        })))
        .mount(mock_server)
        .await;
    Mock::given(method("POST"))
        .and(path_regex(r".*/auth/v4/refresh$"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "UID": "dummy", "AccessToken": "dummy",
            "RefreshToken": "dummy", "Scopes": ["dummy"],
        })))
        .mount(mock_server)
        .await;

    let host: Endpoint = mock_server
        .uri()
        .parse()
        .expect("mock server URI must be valid");
    Session::builder()
        .with_env_id(EnvId::new_custom(MockApiEnv { host }))
        .build()
        .await
        .unwrap()
}
