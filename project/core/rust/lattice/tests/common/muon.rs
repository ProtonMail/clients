use std::future::Future;
use std::pin::Pin;

use async_compat::Compat;
use derive_more::{Debug, Deref};
use futures::TryFutureExt;
use muon::{
    Environment,
    auth::{Auth, Tokens},
    client::builder::Hyper,
    common::{GenericContext, RetryPolicy},
    http::{HttpReq, HttpRes, hyper::connector::HyperConnector},
    rt::{
        InstantFactory, Monotonic, MuonInstant, MuonSystemTime, OperatingSystem, Resolve,
        SinceUnixEpoch, Sleep, SystemTimeFactory, TcpConnect,
    },
    store::WithoutPersistence,
};

use lattice::{LtTransportProvider, LtWireRequest, LtWireRequestProvider};

use lattice::{
    LatticeError, LtApiResponseError, LtContract, LtResponseBody, LtSlimAPIJSON,
    auth::LtAuthApiSession,
};
use lattice_muon2::{LtTransportError, Muon2Transport, Muon2WireRequestProvider};
use lattice_quark::{LtQuarkContract, LtQuarkTransportProvider, jail::unban::LtQuarkJailUnban};
use serde::Deserialize;

use crate::common::test_transport::Muon2TestTransport;

#[derive(Debug, Clone)]
pub struct TimeCapability {
    at_start: std::time::Instant,
}
impl Default for TimeCapability {
    fn default() -> Self {
        Self {
            at_start: std::time::Instant::now(),
        }
    }
}
impl Sleep for TimeCapability {
    type Sleep<'a>
        = Pin<Box<dyn Future<Output = ()> + Send + Sync + 'a>>
    where
        Self: 'a;
    fn sleep(&self, duration: core::time::Duration) -> Self::Sleep<'static> {
        Box::pin(tokio::time::sleep(duration))
    }
}
impl InstantFactory for TimeCapability {
    type Instant = MuonInstant;
    fn now(&self) -> Self::Instant {
        MuonInstant::from_duration(std::time::Instant::now() - self.at_start)
    }
}
unsafe impl Monotonic for TimeCapability {}
impl SystemTimeFactory for TimeCapability {
    type SystemTime = MuonSystemTime;
    fn now(&self) -> Self::SystemTime {
        MuonSystemTime::since_unix_epoch(
            std::time::SystemTime::now() // nosemgrep
                .duration_since(std::time::UNIX_EPOCH)
                .expect("failed to get time"),
        )
    }
}
#[derive(Debug, Clone, Default)]
pub struct MyTcpConnector;
impl TcpConnect for MyTcpConnector {
    type Socket = Compat<tokio::net::TcpStream>;
    async fn tcp_connect(&self, addr: std::net::SocketAddr) -> Result<Self::Socket, Self::Err> {
        tokio::net::TcpStream::connect(addr).await.map(Compat::new)
    }

    type Err = std::io::Error;
}
#[derive(Debug, Clone, Default)]
pub struct MyResolver;
impl Resolve for MyResolver {
    type Err = std::io::Error;
    fn resolve(
        &self,
        host: &str,
    ) -> impl std::future::Future<Output = std::result::Result<Vec<core::net::IpAddr>, Self::Err>>
    {
        tokio::net::lookup_host(format!("{host}:80"))
            .map_ok(|addresses| addresses.map(|addr| addr.ip()).collect())
    }
}
#[derive(Debug, Clone, Default)]
pub struct MyOperatingSystem {
    time: TimeCapability,
    dialer: MyTcpConnector,
    resolver: MyResolver,
}
impl OperatingSystem for MyOperatingSystem {
    type Resolver = MyResolver;
    type TcpConnector = MyTcpConnector;
    type Time = TimeCapability;
    fn get_time_capabilities(&self) -> &Self::Time {
        &self.time
    }
    fn get_tcp_connector(&self) -> &Self::TcpConnector {
        &self.dialer
    }
    fn get_resolver(&self) -> &Self::Resolver {
        &self.resolver
    }
}
#[derive(Debug, Clone)]
pub struct TokioExecutor;
impl futures::task::Spawn for TokioExecutor {
    fn spawn_obj(
        &self,
        future: futures::task::FutureObj<'static, ()>,
    ) -> Result<(), futures::task::SpawnError> {
        std::mem::drop(tokio::spawn(future));
        Ok(())
    }
}

pub type MuonCtx = GenericContext<
    HyperConnector<MyOperatingSystem, muon::rt::SendExecutor<TokioExecutor>>,
    WithoutPersistence<()>,
    muon::NoInfo,
>;
pub type Client = muon::Client<MuonCtx>;

pub fn environment() -> muon::Environment {
    match std::env::var("ENV_NAME") {
        Ok(name) => Environment::new_atlas_name(name),
        Err(std::env::VarError::NotPresent) => Environment::new_atlas(),
        Err(std::env::VarError::NotUnicode(e)) => panic!("{}", e.display()),
    }
}

pub fn new_client() -> Client {
    let env = environment();
    let app = muon::App::new("ios-pass@1.17.0").unwrap();
    let builder = muon::Client::builder_with_transport::<Hyper>(app, env)
        .with_operating_system(MyOperatingSystem::default(), rand::rng())
        .with_multi_thread_executor(TokioExecutor);
    builder
        .retry_policy(RetryPolicy::default())
        .without_persistence()
        .without_cookie_store()
        .build()
        .unwrap()
}

pub async fn generate_muon_session() -> Session {
    let client = new_client();

    Session(client.new_session_without_credentials(()).await.unwrap())
}

#[derive(Deref, Debug)]
pub struct Session(pub(super) muon::Session<MuonCtx>);

impl Session {
    pub async fn swap_session(self, api_session: LtAuthApiSession) -> Self {
        let client = self.client().clone();
        let _ = self.0.remove_auth().await.unwrap();

        let credentials = Auth::internal(
            api_session.user_id,
            api_session.id,
            Tokens::access(
                api_session.access_token.into_inner(),
                api_session.refresh_token.into_inner(),
                api_session.scopes,
            ),
        );
        Session::from_session(
            client
                .new_session_with_credentials((), credentials.try_into().unwrap())
                .await
                .unwrap(),
        )
    }
    pub fn from_session(session: muon::Session<MuonCtx>) -> Self {
        Self(session)
    }

    pub fn into_inner(self) -> muon::Session<MuonCtx> {
        self.0
    }

    pub async fn clone(&self) -> Self {
        Self(self.0.client().get_session(()).await.unwrap())
    }

    pub async fn send_lt_generic<E: LtResponseBody, T: LtContract<Response = E>>(
        &self,
        req: T,
    ) -> Result<E, LtTransportError> {
        Muon2TestTransport::new(self.0.clone())
            .send_contract_request(&req)
            .await
    }

    pub async fn send_lt<
        E: for<'de> Deserialize<'de>,
        T: LtContract<Response = LtSlimAPIJSON<E>>,
    >(
        &self,
        req: T,
    ) -> Result<E, LtTransportError> {
        self.send_lt_generic(req).await.map(|e| e.0)
    }

    pub async fn send_quark<T: LtQuarkContract>(
        &self,
        req: T,
    ) -> Result<T::Response, LtTransportError> {
        Muon2TestTransport::new(self.0.clone())
            .send_contract_quark(&req)
            .await
    }
}
