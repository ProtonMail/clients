use crate::proton_layers::CookieJarLayer;
use crate::proton_layers::SetCryptoClockLayer;
use crate::proton_layers::SetDefaultServiceTypeLayer;
use crate::proton_layers::SetDefaultTimeoutLayer;
use crate::proton_store::MuonStoreImpl;
use crate::session::Config;
use crate::store::Store;
use crate::verification::ChallengeNotifierLayer;
use crate::verification::DynChallengeNotifier;
use cookie::CookieJar;
use mail_muon::App;
use mail_muon::client::InfoProvider;
use mail_muon::client::middleware::{DisplayLogger, Tagger};
use mail_muon::common::ConstProxy;
use mail_muon::common::IntoDyn;
use mail_muon::common::ParseEndpointErr;
use mail_muon::dns::{GoogleDoh, Quad9Doh};
use mail_muon::error::ParseAppVersionErr;
use mail_task_service::Tokio;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;

/// An error that can occur when building a Proton client.
#[derive(Debug, Error)]
pub enum BuildError {
    /// The app version could not be parsed.
    #[error(transparent)]
    ParseAppVersion(#[from] ParseAppVersionErr),

    /// A proxy endpoint could not be parsed.
    #[error(transparent)]
    ParseEndpoint(#[from] ParseEndpointErr),

    /// The client could not be built.
    #[error(transparent)]
    Build(#[from] mail_muon::Error),
}

/// Builds a new Proton client.
pub async fn build<S: Store>(
    config: &Arc<Config>,
    store: &Arc<RwLock<S>>,
    notifier: DynChallengeNotifier,
    info_provider: Option<Arc<dyn InfoProvider>>,
    allow_doh: bool,
) -> Result<mail_muon::Client, BuildError> {
    use mail_muon::rt::{AsyncResolver, ResolverExt, with_fallback};

    let store = MuonStoreImpl::new(&config.env_id, store);

    let app = if let Some(agent) = &config.user_agent {
        App::new(&config.app_version)?.with_user_agent(agent)
    } else {
        App::new(&config.app_version)?
    };

    let mut builder = (mail_muon::Client::builder_async(app, store).await)
        .layer_front(Tagger::default())
        .layer_back(SetCryptoClockLayer)
        .layer_back(SetDefaultServiceTypeLayer)
        .layer_back(SetDefaultTimeoutLayer)
        .layer_back(ChallengeNotifierLayer::new(notifier))
        .layer_back(CookieJarLayer::new(CookieJar::new()))
        .layer_back(DisplayLogger::debug())
        .spawner(Tokio::spawner());

    if let Some(resolver) = config.resolver.clone() {
        builder = builder.resolver(resolver.layer([with_fallback(AsyncResolver)]));
    }

    if let Some(proxy) = &config.proxy {
        builder = builder.proxy(ConstProxy::new(proxy.parse()?));
    }

    if allow_doh {
        builder = builder.doh([Quad9Doh.into_dyn(), GoogleDoh.into_dyn()]);
    }

    let mut client = builder.build()?;

    if let Some(info_provider) = info_provider {
        client = client.with_info_provider(info_provider);
    }

    Ok(client)
}
