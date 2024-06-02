use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use figment::Figment;
use hickory_resolver::{error::ResolveError, TokioAsyncResolver};
use rocket_db_pools::{Database, Pool};
use rocket_okapi::{
    gen::OpenApiGenerator,
    request::{OpenApiFromRequest, RequestHeaderInput},
};
use rocket_sync_db_pools::database;
use tigerbeetle_unofficial as tb;

pub mod api;
pub mod process;

#[database("meta")]
pub struct MetaConn(pub diesel::SqliteConnection);

impl<'r> OpenApiFromRequest<'r> for MetaConn {
    fn from_request_input(
        _gen: &mut OpenApiGenerator,
        _name: String,
        _required: bool,
    ) -> rocket_okapi::Result<RequestHeaderInput> {
        Ok(RequestHeaderInput::None)
    }
}

#[derive(Database)]
#[database("orders")]
pub struct Orders(pub rocket_db_pools::deadpool_redis::Pool);

#[derive(Database)]
#[database("accounting")]
pub struct Accounting(pub AccountingPool);

#[derive(Clone)]
pub struct AccountingPool {
    clients: Vec<Arc<tb::Client>>,
    counter: Arc<AtomicUsize>,
}

#[derive(Debug, thiserror::Error)]
pub enum AccountingError {
    #[error("couldn't read TigerBeetle config: {0}")]
    Config(figment::Error),
    #[error("resolve url: {0}")]
    Resolve(ResolveError),
    #[error("no port")]
    NoPort,
    #[error("connect to TigerBeetle {0}")]
    Connect(tb::error::NewClientError),
}

#[async_trait::async_trait]
impl Pool for AccountingPool {
    type Connection = Arc<tb::Client>;

    type Error = AccountingError;

    async fn init(figment: &Figment) -> Result<Self, Self::Error> {
        let config: rocket_db_pools::Config = figment.extract().map_err(AccountingError::Config)?;

        let resolver =
            TokioAsyncResolver::tokio_from_system_conf().map_err(AccountingError::Resolve)?;

        let (domain, port) = config.url.split_once(':').ok_or(AccountingError::NoPort)?;
        // todo: try to implement dns re-resolution
        let clients = resolver
            .ipv4_lookup(domain)
            .await
            .map_err(AccountingError::Resolve)?
            .iter()
            .map(|ip| {
                tb::Client::new(0, format!("{ip}:{port}"), config.max_connections as u32)
                    .map(Arc::new)
                    .map_err(AccountingError::Connect)
            })
            .collect::<Result<Vec<_>, AccountingError>>()?;

        tracing::info!("got tigerbeetle cxns");

        Ok(AccountingPool {
            clients,
            counter: Arc::default(),
        })
    }

    async fn get(&self) -> Result<Self::Connection, Self::Error> {
        Ok(Arc::clone(
            &self.clients[self.counter.fetch_add(1, Ordering::SeqCst) & self.clients.len()],
        ))
    }

    async fn close(&self) {}
}
