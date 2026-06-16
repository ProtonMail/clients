use std::net::IpAddr as StdIpAddr;
use std::sync::Arc;

use parking_lot::Mutex;

use crate::core::effect_channel::{EffectChannel, EffectChannelHandler, Responder};
use crate::errors::ProtonError;
use crate::errors::unexpected::UnexpectedError;
use mail_common::{MailContextError, ProtonMailError as RealProtonMailError};
use mail_muon::Result as MuonResult;
use mail_muon::common::{Addr, Host, IntoDyn, Name};
use mail_muon::rt::{DynResolver, ResolveRes as MuonResolveRes, Resolver as MuonResolver};
use mail_muon::util::IntoIterExt;
use tokio_util::sync::CancellationToken;

#[derive(uniffi::Enum)]
pub enum IpAddr {
    V4(String),
    V6(String),
}

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum ResolverError {
    #[error("Failed to resolve due to lack of network")]
    Network(String),
    #[error("{0}")]
    Other(String),
}

impl From<uniffi::UnexpectedUniFFICallbackError> for ResolverError {
    fn from(value: uniffi::UnexpectedUniFFICallbackError) -> Self {
        Self::Other(value.to_string())
    }
}

#[async_trait::async_trait]
pub trait Resolver: Send + Sync {
    /// Resolve the given host to a set of IP addresses.
    async fn resolve(&self, host: String) -> Result<Option<Vec<IpAddr>>, ResolverError>;
}

impl std::fmt::Debug for dyn Resolver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Resolver")
    }
}

#[derive(Debug)]
pub struct ResolverImpl(Arc<dyn Resolver>);

impl ResolverImpl {
    pub fn new(resolver: Arc<dyn Resolver>) -> Self {
        Self(resolver)
    }

    async fn resolve_direct(&self, name: &Name) -> MuonResult<MuonResolveRes> {
        let mut res = Vec::new();

        let addrs = match self.0.resolve(name.to_string()).await {
            Ok(None) => return Ok(MuonResolveRes::None),
            Ok(Some(addrs)) => addrs,
            Err(e) => {
                return Err(mail_muon::Error::other(Box::new(e)));
            }
        };

        for addr in addrs {
            match addr {
                IpAddr::V4(addr) => {
                    if let Ok(addr) = addr.parse() {
                        res.push(Addr::new(name.to_owned(), StdIpAddr::V4(addr)));
                    }
                }

                IpAddr::V6(addr) => {
                    if let Ok(addr) = addr.parse() {
                        res.push(Addr::new(name.to_owned(), StdIpAddr::V6(addr)));
                    }
                }
            }
        }

        if let Some((head, tail)) = res.into_head_tail() {
            Ok(MuonResolveRes::Some(head, tail.collect()))
        } else {
            Ok(MuonResolveRes::None)
        }
    }
}

#[async_trait::async_trait]
impl MuonResolver for ResolverImpl {
    async fn resolve(&self, host: &Host) -> MuonResult<MuonResolveRes> {
        if let Host::Direct(name) = host {
            self.resolve_direct(name).await
        } else {
            Ok(MuonResolveRes::None)
        }
    }
}

/// The answer the foreign side delivers for a resolution request.
///
/// A distinct enum rather than `Result<Option<Vec<IpAddr>>, ResolverError>` because uniffi
/// cannot lift a `Result` (or a `uniffi::Error`) as a method argument.
#[derive(uniffi::Enum)]
pub enum ResolverOutcome {
    /// Resolution succeeded; `None` means "no opinion, fall back to the system resolver".
    Resolved { addrs: Option<Vec<IpAddr>> },
    /// Resolution failed because the network was unavailable.
    NetworkError { message: String },
    /// Resolution failed for any other reason.
    OtherError { message: String },
}

type ResolverResult = Result<Option<Vec<IpAddr>>, ResolverError>;

impl From<ResolverOutcome> for ResolverResult {
    fn from(outcome: ResolverOutcome) -> Self {
        match outcome {
            ResolverOutcome::Resolved { addrs } => Ok(addrs),
            ResolverOutcome::NetworkError { message } => Err(ResolverError::Network(message)),
            ResolverOutcome::OtherError { message } => Err(ResolverError::Other(message)),
        }
    }
}

/// A [`Resolver`] that asks the foreign side over a channel instead of calling it.
struct ChannelResolver {
    channel: EffectChannel<String, ResolverResult>,
}

#[async_trait::async_trait]
impl Resolver for ChannelResolver {
    async fn resolve(&self, host: String) -> ResolverResult {
        // A dead loop (teardown) surfaces as a resolution error; muon's fallback resolver
        // then takes over, so DNS keeps working.
        match self.channel.request(host).await {
            Ok(result) => result,
            Err(e) => Err(ResolverError::Other(e.to_string())),
        }
    }
}

pub(crate) fn resolver_channel() -> (DynResolver, EffectChannelHandler<String, ResolverResult>) {
    let (channel, handler) = EffectChannel::new();
    let resolver = ResolverImpl::new(Arc::new(ChannelResolver { channel })).into_dyn();
    (resolver, handler)
}

/// The foreign side's view of the resolver channel: poll [`next_async`] for a hostname to
/// resolve, answer it, repeat for the session's lifetime.
#[derive(uniffi::Object)]
pub struct ResolverRequestStream {
    handler: EffectChannelHandler<String, ResolverResult>,
    token: CancellationToken,
}

impl ResolverRequestStream {
    pub(crate) fn new(
        handler: EffectChannelHandler<String, ResolverResult>,
        token: CancellationToken,
    ) -> Arc<Self> {
        Arc::new(Self { handler, token })
    }
}

#[uniffi_export]
impl ResolverRequestStream {
    /// Wait for the next hostname to resolve. Resolves to the request to answer, or errors
    /// when the stream is cancelled or the requester is gone.
    #[tracing::instrument(name = "ResolverRequestStream::next", skip_all)]
    pub async fn next_async(self: Arc<Self>) -> Result<Arc<ResolverRequest>, ProtonError> {
        let pending = self
            .token
            .run_until_cancelled(self.handler.next())
            .await
            .ok_or_else(|| {
                tracing::info!("Resolver request stream cancelled");
                RealProtonMailError::from(MailContextError::TaskCancelled)
            })?
            .map_err(|e| {
                tracing::error!("Resolver requester is gone: {e}");
                ProtonError::Unexpected(UnexpectedError::Internal)
            })?;

        let (host, responder) = pending.split();
        Ok(Arc::new(ResolverRequest {
            host,
            responder: Mutex::new(Some(responder)),
        }))
    }

    /// Stop the stream; a pending or future `next_async` resolves to a cancellation error.
    pub fn cancel(&self) {
        tracing::info!("Cancelling resolver request stream");
        self.token.cancel();
    }
}

/// A single outstanding resolution request. The foreign side reads [`host`], then calls
/// [`respond`] exactly once.
#[derive(uniffi::Object)]
pub struct ResolverRequest {
    host: String,
    // `Option` because the answer is one-shot and exported methods only get `&self`.
    responder: Mutex<Option<Responder<ResolverResult>>>,
}

#[uniffi_export]
impl ResolverRequest {
    /// The hostname to resolve.
    #[must_use]
    pub fn host(&self) -> String {
        self.host.clone()
    }

    /// Answer the request with the resolution outcome.
    pub fn respond(&self, outcome: ResolverOutcome) {
        let Some(responder) = self.responder.lock().take() else {
            tracing::warn!("ResolverRequest::respond called more than once; ignoring");
            return;
        };

        if let Err(e) = responder.respond(outcome.into()) {
            tracing::debug!("resolver requester gone before responding: {e}");
        }
    }
}
