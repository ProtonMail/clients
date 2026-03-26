//! Blanket implementations of [`ProtonAuth`] and [`ProtonAccount`] for any
//! [`Sender<ProtonRequest, ProtonResponse>`](mail_muon::common::Sender) type.

use crate::protocol::proton::{
    AddressId, GetAddressResponse, GetAddressesResponse, GetCaptchaOptions, GetKeysAllOptions,
    GetKeysSaltsResponse, GetSessionsUuidResponse, GetSettingsResponse, GetUsersResponse,
    PostAuthInfoRequest, PostAuthInfoResponse, ProtonAccount, ProtonAuth,
};
use mail_api_shared::ApiServiceResult;
use mail_muon::common::Sender;
use mail_muon::http::HttpReqExt;
use mail_muon::{GET, POST, ProtonRequest, ProtonResponse, serde_to_query};
use proton_crypto_account::keys::APIPublicAddressKeys;

const CORE_V4: &str = "/core/v4";
const AUTH_V4: &str = crate::AUTH_V4;

impl<This: ?Sized + Sender<ProtonRequest, ProtonResponse>> ProtonAuth for This {
    async fn get_sessions_uuid(&self) -> ApiServiceResult<GetSessionsUuidResponse> {
        Ok(GET!("{AUTH_V4}/sessions/uuid")
            .send_with(self)
            .await?
            .ok()?
            .into_body_json()?)
    }

    async fn post_auth_info(
        &self,
        request: PostAuthInfoRequest,
    ) -> ApiServiceResult<PostAuthInfoResponse> {
        Ok(POST!("{AUTH_V4}/info")
            .body_json(request)?
            .send_with(self)
            .await?
            .ok()?
            .into_body_json()?)
    }
}

impl<This: ?Sized + Sender<ProtonRequest, ProtonResponse>> ProtonAccount for This {
    async fn get_addresses(&self) -> ApiServiceResult<GetAddressesResponse> {
        Ok(GET!("{CORE_V4}/addresses")
            .send_with(self)
            .await?
            .ok()?
            .into_body_json()?)
    }

    async fn get_address_by_id(&self, id: AddressId) -> ApiServiceResult<GetAddressResponse> {
        Ok(GET!("{CORE_V4}/addresses/{id}")
            .send_with(self)
            .await?
            .ok()?
            .into_body_json()?)
    }

    async fn get_captcha(&self, options: GetCaptchaOptions) -> ApiServiceResult<String> {
        Ok(GET!("{CORE_V4}/captcha")
            .query(serde_to_query(options)?)
            .send_with(self)
            .await?
            .ok()?
            .into_body_string()?)
    }

    async fn get_keys_all(
        &self,
        options: GetKeysAllOptions,
    ) -> ApiServiceResult<APIPublicAddressKeys> {
        Ok(GET!("{CORE_V4}/keys/all")
            .query(serde_to_query(options)?)
            .send_with(self)
            .await?
            .ok()?
            .into_body_json()?)
    }

    async fn get_keys_salts(&self) -> ApiServiceResult<GetKeysSaltsResponse> {
        Ok(GET!("{CORE_V4}/keys/salts")
            .send_with(self)
            .await?
            .ok()?
            .into_body_json()?)
    }

    async fn get_settings(&self) -> ApiServiceResult<GetSettingsResponse> {
        Ok(GET!("{CORE_V4}/settings")
            .send_with(self)
            .await?
            .ok()?
            .into_body_json()?)
    }

    async fn get_users(&self) -> ApiServiceResult<GetUsersResponse> {
        Ok(GET!("{CORE_V4}/users")
            .send_with(self)
            .await?
            .ok()?
            .into_body_json()?)
    }
}
