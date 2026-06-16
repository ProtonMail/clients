use crate::core::effect_channel::{EffectChannel, EffectChannelHandler, Responder};
use crate::errors::ProtonError;
use crate::errors::unexpected::UnexpectedError;
use mail_common::{MailContextError, ProtonMailError as RealProtonMailError};
use mail_core_common::device as device_info;
use parking_lot::Mutex;
use std::ops::Deref;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

pub type DynDeviceInfoProvider = Arc<dyn DeviceInfoProvider>;

/// The response to a device info request.
#[derive(Debug, uniffi::Record)]
#[allow(clippy::large_enum_variant)]
pub struct DeviceInfo {
    /// The language code of this Locale.
    language: String,
    /// Time zone id, such as "Asia/Calcutta", "GMT+5:30" or "PST".
    timezone: String,
    /// Time zone raw offset in minutes from GMT including daylight saving.
    timezone_offset: i32,
    /// The end-user-visible name for the end product.
    model: String,
    /// The consumer-visible brand with which the product/hardware will be associated.
    brand: String,
    /// The name of the industrial design.
    codename: String,
    /// The device's UUID.
    uuid: String,
    /// The country/region code, in ISO 3166 2-letter code, or a UN M.49 3-digit code.
    country: String,
    /// If device/OS is rooted/jailbroken.
    rooted: bool,
    /// The current scaling factor for fonts, relative to the base density scaling.
    font_scale: String,
    /// The total size of the device storage in GB.
    storage: f64,
    /// If the device (or current context) is using dark mode.
    dark_mode: bool,
    /// List of enabled input methods application name (e.g. packageName, bundle id).
    keyboards: Vec<String>,
}
impl From<DeviceInfo> for device_info::DeviceInfo {
    fn from(response: DeviceInfo) -> Self {
        Self {
            language: response.language,
            timezone: response.timezone,
            timezone_offset: response.timezone_offset,
            model: response.model,
            brand: response.brand,
            codename: response.codename,
            uuid: response.uuid,
            country: response.country,
            rooted: response.rooted,
            font_scale: response.font_scale,
            storage: response.storage,
            dark_mode: response.dark_mode,
            keyboards: response.keyboards,
        }
    }
}

/// An interface to provide device info from native.
///
/// No longer exported across FFI: the foreign side answers through [`DeviceInfoRequestStream`]
/// instead of implementing this trait directly. It stays a Rust trait only so the existing
/// [`DeviceInfoProviderWrap`] adapter and core wiring are reused unchanged. `None` means the
/// info could not be obtained.
#[async_trait::async_trait]
pub trait DeviceInfoProvider: Send + Sync + 'static {
    async fn get_device_info(&self) -> Option<DeviceInfo>;
}

#[async_trait::async_trait]
impl<T: ?Sized + DeviceInfoProvider> DeviceInfoProvider for Arc<T> {
    async fn get_device_info(&self) -> Option<DeviceInfo> {
        self.deref().get_device_info().await
    }
}

/// Wraps a `DeviceInfoProvider` to implement the core `DeviceInfoProvider` trait.
pub(crate) struct DeviceInfoProviderWrap<T> {
    inner: T,
}

impl<T: DeviceInfoProvider> DeviceInfoProviderWrap<T> {
    /// Wrap a `DeviceInfoProvider` to implement the core `DeviceInfoProvider` trait.
    pub fn wrap(inner: T) -> device_info::DynDeviceInfoProvider {
        Arc::new(Self { inner })
    }
}

#[async_trait::async_trait]
impl<T: DeviceInfoProvider> device_info::DeviceInfoProvider for DeviceInfoProviderWrap<T> {
    async fn get_device_info(&self) -> Option<device_info::DeviceInfo> {
        self.inner.get_device_info().await.map(Into::into)
    }
}

/// A [`DeviceInfoProvider`] that asks the foreign side over a channel instead of calling it.
///
/// Every device-info request becomes a [`DeviceInfoRequest`] the foreign loop answers; the
/// answer comes back as a `respond` argument, sidestepping the FFI return path that crashes.
struct ChannelDeviceInfoProvider {
    channel: EffectChannel<(), DeviceInfo>,
}

#[async_trait::async_trait]
impl DeviceInfoProvider for ChannelDeviceInfoProvider {
    async fn get_device_info(&self) -> Option<DeviceInfo> {
        // A dead loop (only reachable during teardown, when the requester is being dropped)
        // surfaces as `None` so no device-derived headers are attached.
        self.channel
            .request(())
            .await
            .inspect_err(|e| tracing::error!("device info channel closed: {e}"))
            .ok()
    }
}

pub(crate) fn device_info_channel() -> (
    device_info::DynDeviceInfoProvider,
    EffectChannelHandler<(), DeviceInfo>,
) {
    let (channel, handler) = EffectChannel::new();
    let provider = DeviceInfoProviderWrap::wrap(ChannelDeviceInfoProvider { channel });
    (provider, handler)
}

/// The foreign side's view of the device-info channel: poll [`next_async`] for a request,
/// answer it, repeat for the session's lifetime.
#[derive(uniffi::Object)]
pub struct DeviceInfoRequestStream {
    handler: EffectChannelHandler<(), DeviceInfo>,
    token: CancellationToken,
}

impl DeviceInfoRequestStream {
    pub(crate) fn new(
        handler: EffectChannelHandler<(), DeviceInfo>,
        token: CancellationToken,
    ) -> Arc<Self> {
        Arc::new(Self { handler, token })
    }
}

#[uniffi_export]
impl DeviceInfoRequestStream {
    /// Wait for Rust to ask for device info. Resolves to the request to answer, or errors
    /// when the stream is cancelled or the requester is gone (the loop should then stop).
    #[tracing::instrument(name = "DeviceInfoRequestStream::next", skip_all)]
    pub async fn next_async(self: Arc<Self>) -> Result<Arc<DeviceInfoRequest>, ProtonError> {
        let pending = self
            .token
            .run_until_cancelled(self.handler.next())
            .await
            .ok_or_else(|| {
                tracing::info!("Device info request stream cancelled");
                RealProtonMailError::from(MailContextError::TaskCancelled)
            })?
            .map_err(|e| {
                tracing::error!("Device info requester is gone: {e}");
                ProtonError::Unexpected(UnexpectedError::Internal)
            })?;

        let (_request, responder) = pending.split();
        Ok(Arc::new(DeviceInfoRequest {
            responder: Mutex::new(Some(responder)),
        }))
    }

    /// Stop the stream; a pending or future `next_async` resolves to a cancellation error.
    pub fn cancel(&self) {
        tracing::info!("Cancelling device info request stream");
        self.token.cancel();
    }
}

/// A single outstanding device-info request. The foreign side calls [`respond`] exactly once.
#[derive(uniffi::Object)]
pub struct DeviceInfoRequest {
    // `Option` because the answer is one-shot and exported methods only get `&self`, so the
    // responder must be moved out on the single `respond` call.
    responder: Mutex<Option<Responder<DeviceInfo>>>,
}

#[uniffi_export]
impl DeviceInfoRequest {
    /// Answer the request with the device's info.
    pub fn respond(&self, info: DeviceInfo) {
        let Some(responder) = self.responder.lock().take() else {
            tracing::warn!("DeviceInfoRequest::respond called more than once; ignoring");
            return;
        };

        if let Err(e) = responder.respond(info) {
            tracing::debug!("device info requester gone before responding: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_device_info() -> DeviceInfo {
        DeviceInfo {
            language: "en".into(),
            timezone: "UTC".into(),
            timezone_offset: 0,
            model: String::new(),
            brand: String::new(),
            codename: String::new(),
            uuid: String::new(),
            country: "CH".into(),
            rooted: false,
            font_scale: "1.0".into(),
            storage: 0.0,
            dark_mode: false,
            keyboards: vec![],
        }
    }

    #[tokio::test]
    async fn provider_returns_answered_info() {
        let (channel, handler) = EffectChannel::new();
        let provider = ChannelDeviceInfoProvider { channel };

        let answerer = tokio::spawn(async move {
            let (_, responder) = handler.next().await.unwrap().split();
            responder.respond(sample_device_info()).unwrap();
        });

        let info = provider.get_device_info().await;
        assert_eq!(info.map(|i| i.country), Some("CH".to_string()));
        answerer.await.unwrap();
    }

    #[tokio::test]
    async fn provider_returns_none_when_no_loop_answers() {
        let (channel, handler) = EffectChannel::new();
        let provider = ChannelDeviceInfoProvider { channel };
        drop(handler);

        assert!(provider.get_device_info().await.is_none());
    }
}
