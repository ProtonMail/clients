use std::pin::Pin;

use async_compat::Compat;
use futures::TryFutureExt;
use muon::{
    Environment,
    client::builder::Hyper,
    common::{GenericContext, RetryPolicy},
    http::hyper::connector::HyperConnector,
    rt::{
        InstantFactory, Monotonic, MuonInstant, MuonSystemTime, OperatingSystem, Resolve,
        SinceUnixEpoch, Sleep, SystemTimeFactory, TcpConnect,
    },
    store::WithoutPersistence,
};

use lattice::{
    LatticeError, LtContract,
    muon::LtContractExt,
    quark::{LtQuarkContract, LtQuarkContractExt},
};

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
pub type Session = muon::Session<MuonCtx>;
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
    let app = muon::App::new("android-mail@99.9.40.0-dev").unwrap();
    let builder = muon::Client::builder_with_transport::<Hyper>(app, env)
        .with_operating_system(MyOperatingSystem::default(), rand::rng())
        .with_multi_thread_executor(TokioExecutor);
    builder
        .retry_policy(RetryPolicy::default())
        .without_persistence()
        .build()
        .unwrap()
}

pub async fn generate_muon_session() -> Session {
    let client = new_client();

    client.new_session_without_credentials(()).await.unwrap()
}

pub(crate) trait SessionExt {
    async fn send_lt<T: LtContract>(&self, req: T) -> Result<T::Response, LatticeError>;
    async fn send_quark<T: LtQuarkContract>(&self, req: T) -> Result<T::Response, LatticeError>;
}

impl SessionExt for Session {
    async fn send_lt<T: LtContract>(&self, req: T) -> Result<T::Response, LatticeError> {
        let http_req = req.to_muon_req()?;
        let response = self.send(http_req).await.map_err(LatticeError::Muon)?;
        T::from_muon_res(&response)
    }

    async fn send_quark<T: LtQuarkContract>(&self, req: T) -> Result<T::Response, LatticeError> {
        let http_req = req.to_muon_req()?;
        let response = self.send(http_req).await.map_err(LatticeError::Muon)?;
        T::from_muon_res(&response)
    }
}
