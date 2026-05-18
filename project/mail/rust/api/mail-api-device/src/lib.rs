use mail_api_shared::ApiServiceResult;
use mail_muon::common::Sender;
use mail_muon::http::HttpReqExt;
use mail_muon::{POST, ProtonRequest, ProtonResponse};
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};

const CORE_V4: &str = "/core/v4";

/// In which environment to register the device for push notifications.
#[derive(
    Clone,
    Copy,
    Debug,
    Deserialize_repr,
    Serialize_repr,
    Eq,
    Hash,
    PartialEq
)]
#[repr(u8)]
pub enum DeviceEnvironment {
    Google = 4,
    AppleProd = 6,
    AppleBeta = 7,
    AppleProdET = 14,
    AppleDevET = 15,
    AppleDev = 16,
}

/// Represents `POST /devices` request body.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct RegisterDeviceRequest {
    pub device_token: String,
    pub environment: DeviceEnvironment,
    pub public_key: Option<String>,
    pub ping_notification_status: Option<i32>,
    pub push_notification_status: Option<i32>,
}

#[allow(async_fn_in_trait)]
pub trait DeviceApi {
    async fn register_device(&self, body: RegisterDeviceRequest) -> ApiServiceResult<()>;
}

impl<This: ?Sized + Sender<ProtonRequest, ProtonResponse>> DeviceApi for This {
    async fn register_device(&self, body: RegisterDeviceRequest) -> ApiServiceResult<()> {
        POST!("{CORE_V4}/devices")
            .body_json(body)?
            .send_with(self)
            .await?
            .ok()?;
        Ok(())
    }
}
