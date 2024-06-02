use std::{ops::Deref, sync::Arc};

use figment::Figment;
use hickory_resolver::{error::ResolveError, TokioAsyncResolver};
use itertools::Itertools;
use rocket_db_pools::{Database, Pool};
use tigerbeetle_unofficial as tb;

pub mod api;
pub mod process;

#[derive(Database)]
#[database("meta")]
pub struct Meta(pub rocket_db_pools::diesel::PgPool);

#[derive(Database)]
#[database("orders")]
pub struct Orders(pub rocket_db_pools::deadpool_redis::Pool);

#[derive(Database)]
#[database("accounting")]
pub struct Accounting(pub AccountingPool);

#[derive(Clone)]
pub struct AccountingPool(Arc<tb::Client>);

impl Deref for AccountingPool {
    type Target = tb::Client;

    fn deref(&self) -> &tb::Client {
        self.0.as_ref()
    }
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
        let addresses = resolver
            .ipv4_lookup(domain)
            .await
            .map_err(AccountingError::Resolve)?
            .into_iter()
            .map(|ip| format!("{ip}:{port}"))
            .join(",");
        tracing::info!("connecting to tigerbeetle: {addresses}");

        tb::Client::new(0, addresses, config.max_connections as u32)
            .map(Arc::new)
            .map(Self)
            .map_err(AccountingError::Connect)
    }

    async fn get(&self) -> Result<Self::Connection, Self::Error> {
        Ok(Arc::clone(&self.0))
    }

    async fn close(&self) {}
}
