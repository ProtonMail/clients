use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chrono::DateTime;
use cookie::{Cookie, CookieJar};
use mail_muon::common::{BoxFut, Sender, SenderLayer, ServiceType};
use mail_muon::{ProtonRequest, ProtonResponse, Result as MuonResult};
use proton_crypto_account::proton_crypto::crypto::UnixTimestamp;
use tokio::sync::RwLock;

use crate::crypto_clock::server_crypto_clock;

pub struct SetCryptoClockLayer;

impl SetCryptoClockLayer {
    async fn on_send<S>(&self, inner: &S, req: ProtonRequest) -> MuonResult<ProtonResponse>
    where
        S: Sender<ProtonRequest, ProtonResponse> + ?Sized,
    {
        let response = inner.send(req).await?;

        if let Some(date) = response
            .headers()
            .get("date")
            .and_then(|response_time_header| response_time_header.to_str().ok())
            .and_then(|response_time| DateTime::parse_from_rfc2822(response_time).ok())
            .and_then(|parsed_server_time| parsed_server_time.timestamp().try_into().ok())
            .map(UnixTimestamp)
        {
            server_crypto_clock().update_clock(date);
        }

        Ok(response)
    }
}

impl SenderLayer<ProtonRequest, ProtonResponse> for SetCryptoClockLayer {
    fn on_send<'a>(
        &'a self,
        inner: &'a dyn Sender<ProtonRequest, ProtonResponse>,
        req: ProtonRequest,
    ) -> BoxFut<'a, MuonResult<ProtonResponse>> {
        Box::pin(self.on_send(inner, req))
    }
}

pub struct SetDefaultServiceTypeLayer;

impl SetDefaultServiceTypeLayer {
    async fn on_send<S>(&self, inner: &S, req: ProtonRequest) -> MuonResult<ProtonResponse>
    where
        S: Sender<ProtonRequest, ProtonResponse> + ?Sized,
    {
        let req = if req.get_service_type().is_none() {
            req.service_type(ServiceType::default(), true)
        } else {
            req
        };

        inner.send(req).await
    }
}

impl SenderLayer<ProtonRequest, ProtonResponse> for SetDefaultServiceTypeLayer {
    fn on_send<'a>(
        &'a self,
        inner: &'a dyn Sender<ProtonRequest, ProtonResponse>,
        req: ProtonRequest,
    ) -> BoxFut<'a, MuonResult<ProtonResponse>> {
        Box::pin(self.on_send(inner, req))
    }
}

pub struct SetDefaultTimeoutLayer;

impl SetDefaultTimeoutLayer {
    async fn on_send<S>(&self, inner: &S, mut req: ProtonRequest) -> MuonResult<ProtonResponse>
    where
        S: Sender<ProtonRequest, ProtonResponse> + ?Sized,
    {
        /// The timeout we set by default.
        const CUSTOM_TIMEOUT: Duration = Duration::from_secs(60);

        /// The timeout `mail_muon` sets by default.
        const MUON_DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

        // NOTE: This is not a bug! Muon logs a warning if no timeout is explicitly set;
        // this workaround sets the timeout explicitly if it was not already set to a
        // non-default value earlier in the layer stack.
        if req.get_allowed_time() == MUON_DEFAULT_TIMEOUT {
            // The request has mail_muon's standard 30s timeout. We bump it here to 60s.
            req = req.allowed_time(CUSTOM_TIMEOUT);
        }

        inner.send(req).await
    }
}

impl SenderLayer<ProtonRequest, ProtonResponse> for SetDefaultTimeoutLayer {
    fn on_send<'a>(
        &'a self,
        inner: &'a dyn Sender<ProtonRequest, ProtonResponse>,
        req: ProtonRequest,
    ) -> BoxFut<'a, MuonResult<ProtonResponse>> {
        Box::pin(self.on_send(inner, req))
    }
}

pub struct CookieJarLayer {
    jar: Arc<RwLock<CookieJar>>,
}

impl CookieJarLayer {
    /// Create a new cookie jar layer.
    #[must_use]
    pub fn new(jar: CookieJar) -> Self {
        Self {
            jar: Arc::new(RwLock::new(jar)),
        }
    }
}

#[allow(clippy::similar_names)]
impl CookieJarLayer {
    async fn on_send<S>(&self, inner: &S, mut req: ProtonRequest) -> MuonResult<ProtonResponse>
    where
        S: Sender<ProtonRequest, ProtonResponse> + ?Sized,
    {
        for cookie in self.jar.read().await.iter() {
            req = req.header(("cookie", cookie.to_string()));
        }

        let res = inner.send(req).await?;

        for cookie in res.headers().get_all("set-cookie") {
            if let Ok(cookie) = cookie.to_str()
                && let Ok(cookie) = Cookie::parse(cookie)
            {
                self.jar.write().await.add(cookie.into_owned());
            }
        }

        Ok(res)
    }
}

impl SenderLayer<ProtonRequest, ProtonResponse> for CookieJarLayer {
    fn on_send<'a>(
        &'a self,
        inner: &'a dyn Sender<ProtonRequest, ProtonResponse>,
        req: ProtonRequest,
    ) -> BoxFut<'a, MuonResult<ProtonResponse>> {
        Box::pin(self.on_send(inner, req))
    }
}

/// Kept as two independent values because they answer different questions for
/// the backend — which device is talking (`Accept-Language`, for localizing
/// unauthenticated flows like signup/login/reset) versus which account is
/// talking (`X-Pm-Locale`, the locale the user chose). Each is optional because its
/// source can genuinely be absent, and the backend must then see no header at
/// all rather than a guessed value.
#[derive(Debug, Default, Clone)]
pub struct LocaleHeaders {
    /// `Accept-Language` header
    /// Absent when the session has no device-info source (e.g. the share
    /// extension), so we send nothing rather than inventing a device locale.
    pub accept_language: Option<String>,
    /// `X-Pm-Locale` header
    /// Absent until the user is authenticated and their settings have synced,
    /// since there is no account locale to speak of before then.
    pub pm_locale: Option<String>,
}

#[async_trait]
pub trait LocaleProvider: Send + Sync {
    async fn locale_headers(&self) -> LocaleHeaders;
}

/// A dynamic [`LocaleProvider`].
pub type DynLocaleProvider = Arc<dyn LocaleProvider>;

/// Re-reads the locale on every send rather than capturing it once, so the
/// backend always sees the locale the user is currently using even though the
/// session outlives any single locale choice.
pub struct SetLocaleHeadersLayer {
    provider: Option<DynLocaleProvider>,
}

impl SetLocaleHeadersLayer {
    #[must_use]
    pub fn new(provider: Option<DynLocaleProvider>) -> Self {
        Self { provider }
    }
}

impl SetLocaleHeadersLayer {
    async fn on_send<S>(&self, inner: &S, mut req: ProtonRequest) -> MuonResult<ProtonResponse>
    where
        S: Sender<ProtonRequest, ProtonResponse> + ?Sized,
    {
        if let Some(provider) = &self.provider {
            let headers = provider.locale_headers().await;

            if let Some(accept_language) = headers.accept_language {
                req = req.header(("accept-language", accept_language));
            }

            if let Some(pm_locale) = headers.pm_locale {
                req = req.header(("x-pm-locale", pm_locale));
            }
        }

        inner.send(req).await
    }
}

impl SenderLayer<ProtonRequest, ProtonResponse> for SetLocaleHeadersLayer {
    fn on_send<'a>(
        &'a self,
        inner: &'a dyn Sender<ProtonRequest, ProtonResponse>,
        req: ProtonRequest,
    ) -> BoxFut<'a, MuonResult<ProtonResponse>> {
        Box::pin(self.on_send(inner, req))
    }
}

#[cfg(test)]
mod tests {
    use super::{DynLocaleProvider, LocaleHeaders, LocaleProvider, SetLocaleHeadersLayer};
    use crate::proton_layers::CookieJarLayer;
    use anyhow::Result;
    use async_trait::async_trait;
    use cookie::CookieJar;
    use mail_muon::GET;
    use mail_muon::test::server::{HTTP, Response, Server};
    use std::sync::Arc;
    use tokio::runtime::Handle;

    struct StubLocaleProvider(LocaleHeaders);

    #[async_trait]
    impl LocaleProvider for StubLocaleProvider {
        async fn locale_headers(&self) -> LocaleHeaders {
            self.0.clone()
        }
    }

    fn stub(accept_language: Option<&str>, pm_locale: Option<&str>) -> DynLocaleProvider {
        Arc::new(StubLocaleProvider(LocaleHeaders {
            accept_language: accept_language.map(str::to_owned),
            pm_locale: pm_locale.map(str::to_owned),
        }))
    }

    /// Sends one request through the layer and returns the recorded
    /// `(accept-language, x-pm-locale)` header values.
    async fn record_ping_headers(
        provider: Option<DynLocaleProvider>,
    ) -> Result<(Option<String>, Option<String>)> {
        let server = Server::new(&Handle::current(), &HTTP)?;
        let client = server
            .builder()
            .layer_back(SetLocaleHeadersLayer::new(provider))
            .build()?;

        let recorder = server.new_recorder();
        client.send(GET!("/tests/ping")).await?.ok()?;

        let request = recorder
            .take()
            .into_iter()
            .find(|req| req.uri().path() == "/tests/ping")
            .unwrap();

        let header = |name: &str| {
            request
                .headers()
                .get(name)
                .map(|value| value.to_str().unwrap().to_owned())
        };

        let accept_language = header("accept-language");
        let pm_locale = header("x-pm-locale");

        server.stop().await?;

        Ok((accept_language, pm_locale))
    }

    #[tokio::test]
    async fn test_locale_headers_both_present() -> Result<()> {
        let (accept_language, pm_locale) =
            record_ping_headers(Some(stub(Some("fr-FR"), Some("fr_FR")))).await?;

        // Set verbatim, without a `;q=` quality suffix.
        assert_eq!(accept_language.as_deref(), Some("fr-FR"));
        assert_eq!(pm_locale.as_deref(), Some("fr_FR"));

        Ok(())
    }

    #[tokio::test]
    async fn test_locale_headers_none_present() -> Result<()> {
        let (accept_language, pm_locale) = record_ping_headers(Some(stub(None, None))).await?;

        assert_eq!(accept_language, None);
        assert_eq!(pm_locale, None);

        Ok(())
    }

    #[tokio::test]
    async fn test_locale_headers_unauthenticated_sends_only_accept_language() -> Result<()> {
        // The unauthenticated case: a device language but no account locale.
        let (accept_language, pm_locale) =
            record_ping_headers(Some(stub(Some("en-US"), None))).await?;

        assert_eq!(accept_language.as_deref(), Some("en-US"));
        assert_eq!(pm_locale, None);

        Ok(())
    }

    #[tokio::test]
    async fn test_locale_headers_no_provider_is_noop() -> Result<()> {
        let (accept_language, pm_locale) = record_ping_headers(None).await?;

        assert_eq!(accept_language, None);
        assert_eq!(pm_locale, None);

        Ok(())
    }

    #[tokio::test]
    async fn test_cookie_jar_roundtrip() -> Result<()> {
        let server = Server::new(&Handle::current(), &HTTP)?;

        server.add_handler(|req| {
            (req.uri().path() == "/tests/cookie-bootstrap").then(|| {
                Response::builder()
                    .status(200)
                    .header("set-cookie", "foo=bar")
                    .body(Default::default())
                    .unwrap()
            })
        });

        let client = server
            .builder()
            .layer_back(CookieJarLayer::new(CookieJar::new()))
            .build()?;

        client.send(GET!("/tests/cookie-bootstrap")).await?.ok()?;

        let recorder = server.new_recorder();

        client.send(GET!("/tests/ping")).await?.ok()?;

        let request = recorder
            .take()
            .into_iter()
            .find(|req| req.uri().path() == "/tests/ping")
            .unwrap();

        let cookie = request
            .headers()
            .get("cookie")
            .unwrap()
            .to_str()?
            .to_owned();

        assert_eq!(cookie, "foo=bar");

        server.stop().await?;

        Ok(())
    }
}
