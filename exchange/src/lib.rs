use std::sync::Arc;

use figment::Figment;
use rocket_db_pools::{Database, Pool};
use serde::Deserialize;
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

#[derive(Debug, Clone, Deserialize)]
struct TbConfig {
    #[serde(default)]
    cluster_id: u32,
    #[serde(default)]
    address: String,
    #[serde(default)]
    concurrency_max: u32,
}

impl Default for TbConfig {
    fn default() -> Self {
        Self {
            cluster_id: 0,
            address: "127.0.0.1:3000".to_string(),
            concurrency_max: 32,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AccountingError {
    #[error("couldn't read TigerBeetle config")]
    Config(figment::Error),
    #[error("connect to TigerBeetle")]
    Connect(tb::error::NewClientError),
}

#[async_trait::async_trait]
impl Pool for AccountingPool {
    type Connection = Self;

    /// The error type returned by [`Self::init()`] and [`Self::get()`].
    type Error = AccountingError;

    async fn init(figment: &Figment) -> Result<Self, Self::Error> {
        let config: TbConfig = figment.extract().map_err(AccountingError::Config)?;

        tb::Client::new(config.cluster_id, config.address, config.concurrency_max)
            .map(|c| AccountingPool(Arc::new(c)))
            .map_err(AccountingError::Connect)
    }

    async fn get(&self) -> Result<Self::Connection, Self::Error> {
        Ok(self.clone())
    }

    async fn close(&self) {}
}
