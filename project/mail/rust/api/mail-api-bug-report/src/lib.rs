use mail_api_shared::ApiServiceResult;
use mail_muon::common::Sender;
use mail_muon::http::HttpReqExt;
use mail_muon::{POST, ProtonRequest, ProtonResponse};
use serde::Serialize;
use std::io::Cursor;

const CORE_V4: &str = "/core/v4";

/// Represents `POST /reports/bug` request body.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct PostReportBug {
    #[serde(rename = "OS")]
    pub os: String,
    #[serde(rename = "OSVersion")]
    pub os_version: String,
    pub client: String,
    pub client_version: String,
    pub client_type: u8,
    pub title: String,
    pub description: String,
    pub username: String,
    pub email: String,
    pub logs: Option<(String, Vec<u8>)>,
}

#[allow(async_fn_in_trait)]
pub trait BugReportApi {
    async fn post_report_bug(&self, body: PostReportBug) -> ApiServiceResult<()>;
}

impl<This: ?Sized + Sender<ProtonRequest, ProtonResponse>> BugReportApi for This {
    async fn post_report_bug(&self, body: PostReportBug) -> ApiServiceResult<()> {
        POST!("{CORE_V4}/reports/bug")
            .multipart(move |mut form| {
                form.add_text("OS", body.os);
                form.add_text("OSVersion", body.os_version);
                form.add_text("Client", body.client);
                form.add_text("ClientVersion", body.client_version);
                form.add_text("ClientType", body.client_type.to_string());
                form.add_text("Title", body.title);
                form.add_text("Description", body.description);
                form.add_text("Username", body.username);
                form.add_text("Email", body.email);
                if let Some((file_name, logs)) = body.logs {
                    form.add_reader_file_with_mime(
                        "ApplicationLogs",
                        Cursor::new(logs),
                        file_name,
                        "application/zip"
                            .parse()
                            .expect("application/zip is a valid MIME type"),
                    );
                }
                form
            })
            .await?
            .send_with(self)
            .await?
            .ok()?;
        Ok(())
    }
}
