use std::{ops::Deref, sync::Arc};

use figment::Figment;
use rocket_db_pools::{Database, Pool};
use tigerbeetle_unofficial as tb;

pub mod api;

#[derive(Database)]
#[database("instruments")]
pub struct Instruments(pub sqlx::SqlitePool);

#[derive(Database)]
#[database("accounting")]
pub struct Accounting(pub AccountingPool);

#[derive(Clone)]
pub struct AccountingPool(Arc<tb::Client>);

impl Deref for AccountingPool {
    type Target = tb::Client;

    fn deref(&self) -> &tb::Client {
        self.0.deref()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AccountingError {
    #[error("couldn't read TigerBeetle config: {0}")]
    Config(figment::Error),
    #[error("connect to TigerBeetle {0}")]
    Connect(tb::error::NewClientError),
}

#[async_trait::async_trait]
impl Pool for AccountingPool {
    type Connection = Arc<tb::Client>;

    type Error = AccountingError;

    async fn init(figment: &Figment) -> Result<Self, Self::Error> {
        let config: rocket_db_pools::Config = figment.extract().map_err(AccountingError::Config)?;
        dbg!(&config);

        tb::Client::new(0, config.url, config.max_connections as u32)
            .map(|c| AccountingPool(Arc::new(c)))
            .map_err(AccountingError::Connect)
    }

    async fn get(&self) -> Result<Self::Connection, Self::Error> {
        Ok(Arc::clone(&self.0))
    }

    async fn close(&self) {}
}
