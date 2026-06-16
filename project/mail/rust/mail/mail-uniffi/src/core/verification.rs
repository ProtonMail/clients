use super::datatypes::AppDetails;
use crate::core::datatypes::ApiConfig;
use crate::core::effect_channel::{EffectChannel, EffectChannelHandler, Responder};
use crate::core::resolver::{ResolverRequestStream, resolver_channel};
use crate::errors::ProtonError;
use crate::errors::unexpected::UnexpectedError;
use crate::uniffi_async;
use futures::{FutureExt, TryFutureExt};
use itertools::Itertools;
use mail_common::{
    MailContextError, ProtonMailError as RealProtonMailError, Unexpected,
    UserApiServiceError as RealUserApiServiceError,
};
use mail_core_api::verification as hv;
use parking_lot::Mutex;
use std::ops::Deref;
use std::sync::Arc;
use tokio_util::sync::{CancellationToken, DropGuard};
use tracing::error;

pub type DynChallengeNotifier = Arc<dyn ChallengeNotifier>;

#[derive(Debug, uniffi::Record)]
pub struct Query {
    pub key: String,
    pub val: Option<String>,
}

impl From<Query> for (String, Option<String>) {
    fn from(q: Query) -> Self {
        (q.key, q.val)
    }
}

impl From<(String, Option<String>)> for Query {
    fn from((key, val): (String, Option<String>)) -> Self {
        Self { key, val }
    }
}

#[derive(Debug, uniffi::Record)]
pub struct Header {
    pub key: String,
    pub val: String,
}

impl From<Header> for (String, String) {
    fn from(h: Header) -> Self {
        (h.key, h.val)
    }
}

impl From<(String, String)> for Header {
    fn from((key, val): (String, String)) -> Self {
        Self { key, val }
    }
}

/// The server of a human verification challenge.
#[derive(Debug, uniffi::Object)]
pub struct ChallengeServer {
    inner: hv::ChallengeServer,
}

impl ChallengeServer {
    fn new(inner: hv::ChallengeServer) -> Arc<Self> {
        Arc::new(Self { inner })
    }
}

#[uniffi_export]
impl ChallengeServer {
    /// The scheme of the server.
    #[must_use]
    pub fn scheme(&self) -> String {
        self.inner.server.scheme().to_string()
    }

    /// The original hostname of the server.
    #[must_use]
    pub fn original_host(&self) -> String {
        self.inner.server.name().to_string()
    }

    /// The resolved hostname of the server.
    #[must_use]
    pub fn resolved_host(&self) -> String {
        self.inner.name.to_string()
    }

    /// The port of the server.
    #[must_use]
    pub fn port(&self) -> u16 {
        self.inner.server.port()
    }

    /// The path of the server.
    #[must_use]
    pub fn path(&self) -> String {
        self.inner.server.path.clone()
    }

    /// Whether alternative routing is enabled.
    #[must_use]
    pub fn doh(&self) -> bool {
        self.original_host() != self.resolved_host()
    }
}

/// The payload of a human verification challenge.
#[derive(Debug, uniffi::Object)]
pub struct ChallengePayload {
    inner: hv::ChallengePayload,
}

impl ChallengePayload {
    fn new(inner: hv::ChallengePayload) -> Arc<Self> {
        Arc::new(Self { inner })
    }
}

#[uniffi_export]
impl ChallengePayload {
    /// The initial human verification token.
    #[must_use]
    pub fn token(&self) -> String {
        self.inner.token.clone()
    }

    /// The verification methods available.
    #[must_use]
    pub fn methods(&self) -> Vec<String> {
        self.inner.methods.clone()
    }

    /// The challenge description.
    #[must_use]
    pub fn description(&self) -> String {
        self.inner.description.clone()
    }

    // The challenge expiration time.
    #[must_use]
    pub fn expires_at(&self) -> u64 {
        self.inner.expires_at
    }

    /// The URL base.
    #[must_use]
    pub fn base(&self) -> String {
        self.inner.base()
    }

    /// The URL path.
    #[must_use]
    pub fn path(&self) -> String {
        self.inner.path().to_owned()
    }

    /// The query parameters of the URL.
    #[must_use]
    pub fn query(&self) -> Vec<Query> {
        self.inner.query().into_iter().map_into().collect()
    }
}

/// The response to a human verification challenge.
#[derive(Debug, uniffi::Enum)]
pub enum ChallengeResponse {
    /// The challenge was successfully completed.
    Success {
        /// The token to submit to the server.
        token: String,
        /// The type of the token.
        ttype: String,
    },

    /// The challenge was not completed.
    Failure,

    /// The challenge was cancelled.
    Cancelled,
}

impl From<ChallengeResponse> for hv::ChallengeResponse {
    fn from(response: ChallengeResponse) -> Self {
        match response {
            ChallengeResponse::Success { token, ttype } => Self::Success { token, ttype },
            ChallengeResponse::Failure => Self::Failure,
            ChallengeResponse::Cancelled => Self::Cancelled,
        }
    }
}

/// A response to an HTTP request sent by the loader.
#[derive(Debug, uniffi::Record)]
pub struct ChallengeLoaderResponse {
    /// The HTTP status code of the response.
    pub status: u16,

    /// The HTTP status text of the response.
    pub reason: Option<String>,

    /// The content type of the response.
    pub content_type: Option<String>,

    /// The content encoding of the response.
    pub content_encoding: Option<String>,

    /// The headers of the response.
    pub headers: Vec<Header>,

    /// The contents of the response.
    pub contents: Vec<u8>,
}

impl From<hv::ChallengeLoaderResponse> for ChallengeLoaderResponse {
    fn from(res: hv::ChallengeLoaderResponse) -> Self {
        Self {
            status: res.status,
            reason: res.reason,
            content_type: res.content_type,
            content_encoding: res.content_encoding,
            headers: res.headers.into_iter().map_into().collect(),
            contents: res.contents.to_vec(),
        }
    }
}

/// The payload of a human verification challenge.
#[derive(uniffi::Object)]
pub struct ChallengeLoader {
    inner: hv::ChallengeLoader,
    // `Some` when a custom resolver was requested. Cancels the resolver stream's token when the
    // loader is dropped — the loader has no parent context, so it is the resolver stream's
    // lifetime root. Held only for its `Drop`.
    _resolver_cancel_on_drop: Option<DropGuard>,
}

/// A freshly created challenge loader, plus the resolver stream the foreign side must drive
/// when `ApiConfig::use_custom_resolver` was set (otherwise `None` and muon resolves itself).
#[derive(uniffi::Record)]
pub struct ChallengeLoaderBundle {
    pub loader: Arc<ChallengeLoader>,
    pub resolver_stream: Option<Arc<ResolverRequestStream>>,
}

/// Create a new `ChallengeLoader`.
#[uniffi_export]
#[tracing::instrument(skip_all)]
pub async fn new_challenge_loader(
    cfg: ApiConfig,
    app: AppDetails,
) -> Result<ChallengeLoaderBundle, ProtonError> {
    let use_custom_resolver = cfg.use_custom_resolver;
    let mut cfg = cfg
        .into_real_api_config(app)
        .inspect_err(|e| error!("{e:?}"))
        .map_err(|_| UnexpectedError::Config)?;

    // Resolver is opt-in. When requested, the loader owns the resolver stream's lifetime, so a
    // token rooted on the loader is the right parent: the stream gets a child, and dropping the
    // loader cancels the root (via the drop guard), cascading to the stream.
    let mut resolver_cancel_on_drop = None;
    let resolver_stream = use_custom_resolver.then(|| {
        let (resolver, handler) = resolver_channel();
        cfg.resolver = Some(resolver);
        let token = CancellationToken::new();
        let stream = ResolverRequestStream::new(handler, token.child_token());
        resolver_cancel_on_drop = Some(token.drop_guard());
        stream
    });

    let inner = uniffi_async(async move {
        hv::ChallengeLoader::new(cfg.into())
            .inspect_err(|e| error!("{e:?}"))
            .map_err(|_| RealProtonMailError::Unexpected(Unexpected::Config))
            .await
    })
    .await?;

    let loader = Arc::new(ChallengeLoader {
        inner,
        _resolver_cancel_on_drop: resolver_cancel_on_drop,
    });

    Ok(ChallengeLoaderBundle {
        loader,
        resolver_stream,
    })
}

#[uniffi_export]
impl ChallengeLoader {
    /// Send a GET request to the server and return the response.
    #[tracing::instrument(skip_all)]
    pub async fn get(
        &self,
        base: String,
        path: String,
        query: Vec<Query>,
        header: Vec<Header>,
    ) -> Result<ChallengeLoaderResponse, ProtonError> {
        let inner = self.inner.clone();
        let query = query.into_iter().map_into();
        let header = header.into_iter().map_into();

        uniffi_async(async move {
            inner
                .get(base, path, query, header)
                .map_err(RealUserApiServiceError::try_from)
                .map_err(|e| match e {
                    Ok(e) => RealProtonMailError::ServerError(e.into()),
                    Err(e) => RealProtonMailError::Unexpected(e.into()),
                })
                .await
        })
        .ok_into()
        .err_into()
        .await
    }

    /// Send a POST request to the server and return the response.
    #[tracing::instrument(skip_all)]
    pub async fn post(
        &self,
        base: String,
        path: String,
        query: Vec<Query>,
        header: Vec<Header>,
        body: Vec<u8>,
    ) -> Result<ChallengeLoaderResponse, ProtonError> {
        let inner = self.inner.clone();
        let query = query.into_iter().map_into();
        let header = header.into_iter().map_into();

        uniffi_async(async move {
            inner
                .post(base, path, query, header, body)
                .map_err(RealUserApiServiceError::try_from)
                .map_err(|e| match e {
                    Ok(e) => RealProtonMailError::ServerError(e.into()),
                    Err(e) => RealProtonMailError::Unexpected(e.into()),
                })
                .await
        })
        .ok_into()
        .err_into()
        .await
    }

    /// Send a PUT request to the server and return the response.
    #[tracing::instrument(skip_all)]
    pub async fn put(
        &self,
        base: String,
        path: String,
        query: Vec<Query>,
        header: Vec<Header>,
        body: Vec<u8>,
    ) -> Result<ChallengeLoaderResponse, ProtonError> {
        let inner = self.inner.clone();
        let query = query.into_iter().map_into();
        let header = header.into_iter().map_into();

        uniffi_async(async move {
            inner
                .put(base, path, query, header, body)
                .map_err(RealUserApiServiceError::try_from)
                .map_err(|e| match e {
                    Ok(e) => RealProtonMailError::ServerError(e.into()),
                    Err(e) => RealProtonMailError::Unexpected(e.into()),
                })
                .await
        })
        .ok_into()
        .err_into()
        .await
    }
}

/// An interface by which human verification challenges can be handled.
#[async_trait::async_trait]
pub trait ChallengeNotifier: Send + Sync + 'static {
    /// Called when a human verification challenge is encountered.
    ///
    /// The `server` is the server that sent the challenge, and it may be a direct server,
    /// such as `mail.proton.me`, or an indirect server (aka alternative routing),
    /// such as `d{base32}.protonpro.xyz`.
    async fn on_challenge(
        &self,
        server: Arc<ChallengeServer>,
        payload: Arc<ChallengePayload>,
    ) -> ChallengeResponse;
}

#[async_trait::async_trait]
impl<T: ?Sized + ChallengeNotifier> ChallengeNotifier for Arc<T> {
    async fn on_challenge(
        &self,
        server: Arc<ChallengeServer>,
        payload: Arc<ChallengePayload>,
    ) -> ChallengeResponse {
        self.deref().on_challenge(server, payload).await
    }
}

/// Wraps a `ChallengeNotifier` to implement the core `ChallengeNotifier` trait.
pub(crate) struct ChallengeNotifierWrap<T> {
    inner: T,
}

impl<T: ChallengeNotifier> ChallengeNotifierWrap<T> {
    /// Wrap a `ChallengeNotifier` to implement the core `ChallengeNotifier` trait.
    pub fn wrap(inner: T) -> hv::DynChallengeNotifier {
        Arc::new(Self { inner })
    }
}

#[async_trait::async_trait]
impl<T: ChallengeNotifier> hv::ChallengeNotifier for ChallengeNotifierWrap<T> {
    async fn on_challenge(
        &self,
        server: hv::ChallengeServer,
        payload: hv::ChallengePayload,
    ) -> hv::ChallengeResponse {
        let server = ChallengeServer::new(server);
        let payload = ChallengePayload::new(payload);

        self.inner.on_challenge(server, payload).map_into().await
    }
}

/// The request side of a human-verification challenge: the server and payload to present.
type ChallengeRequestData = (Arc<ChallengeServer>, Arc<ChallengePayload>);

/// A [`ChallengeNotifier`] that asks the foreign side over a channel instead of calling it.
struct ChannelChallengeNotifier {
    channel: EffectChannel<ChallengeRequestData, ChallengeResponse>,
}

#[async_trait::async_trait]
impl ChallengeNotifier for ChannelChallengeNotifier {
    async fn on_challenge(
        &self,
        server: Arc<ChallengeServer>,
        payload: Arc<ChallengePayload>,
    ) -> ChallengeResponse {
        self.channel
            .request((server, payload))
            .await
            .unwrap_or_else(|e| {
                tracing::error!("challenge channel closed: {e}");
                ChallengeResponse::Cancelled
            })
    }
}

pub(crate) fn challenge_channel() -> (
    hv::DynChallengeNotifier,
    EffectChannelHandler<ChallengeRequestData, ChallengeResponse>,
) {
    let (channel, handler) = EffectChannel::new();
    let notifier = ChallengeNotifierWrap::wrap(ChannelChallengeNotifier { channel });
    (notifier, handler)
}

/// The foreign side's view of the challenge channel: poll [`next_async`] for a challenge to
/// solve, answer it, repeat for the session's lifetime.
#[derive(uniffi::Object)]
pub struct ChallengeRequestStream {
    handler: EffectChannelHandler<ChallengeRequestData, ChallengeResponse>,
    token: CancellationToken,
}

impl ChallengeRequestStream {
    pub(crate) fn new(
        handler: EffectChannelHandler<ChallengeRequestData, ChallengeResponse>,
        token: CancellationToken,
    ) -> Arc<Self> {
        Arc::new(Self { handler, token })
    }
}

#[uniffi_export]
impl ChallengeRequestStream {
    /// Wait for the next human-verification challenge. Resolves to the request to answer, or
    /// errors when the stream is cancelled or the requester is gone.
    #[tracing::instrument(name = "ChallengeRequestStream::next", skip_all)]
    pub async fn next_async(self: Arc<Self>) -> Result<Arc<ChallengeRequest>, ProtonError> {
        let pending = self
            .token
            .run_until_cancelled(self.handler.next())
            .await
            .ok_or_else(|| {
                tracing::info!("Challenge request stream cancelled");
                RealProtonMailError::from(MailContextError::TaskCancelled)
            })?
            .map_err(|e| {
                tracing::error!("Challenge requester is gone: {e}");
                ProtonError::Unexpected(UnexpectedError::Internal)
            })?;

        let ((server, payload), responder) = pending.split();
        Ok(Arc::new(ChallengeRequest {
            server,
            payload,
            responder: Mutex::new(Some(responder)),
        }))
    }

    /// Stop the stream; a pending or future `next_async` resolves to a cancellation error.
    pub fn cancel(&self) {
        tracing::info!("Cancelling challenge request stream");
        self.token.cancel();
    }
}

/// A single outstanding human-verification challenge. The foreign side reads [`server`] and
/// [`payload`], then calls [`respond`] exactly once.
#[derive(uniffi::Object)]
pub struct ChallengeRequest {
    server: Arc<ChallengeServer>,
    payload: Arc<ChallengePayload>,
    // `Option` because the answer is one-shot and exported methods only get `&self`.
    responder: Mutex<Option<Responder<ChallengeResponse>>>,
}

#[uniffi_export]
impl ChallengeRequest {
    #[must_use]
    pub fn server(&self) -> Arc<ChallengeServer> {
        Arc::clone(&self.server)
    }

    #[must_use]
    pub fn payload(&self) -> Arc<ChallengePayload> {
        Arc::clone(&self.payload)
    }

    pub fn respond(&self, response: ChallengeResponse) {
        let Some(responder) = self.responder.lock().take() else {
            tracing::warn!("ChallengeRequest::respond called more than once; ignoring");
            return;
        };

        if let Err(e) = responder.respond(response) {
            tracing::debug!("challenge requester gone before responding: {e}");
        }
    }
}
