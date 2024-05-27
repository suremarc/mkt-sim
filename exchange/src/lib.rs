use std::{ops::Deref, sync::Arc};

use figment::Figment;
use rocket_db_pools::{Database, Pool};
use rocket_okapi::{
    gen::OpenApiGenerator,
    request::{OpenApiFromRequest, RequestHeaderInput},
};
use rocket_sync_db_pools::database;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tigerbeetle_unofficial as tb;

pub mod accounts;
pub mod assets;
pub mod auth;
#[rustfmt::skip]
pub mod schema;
pub mod orders;
pub mod types;

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

        tb::Client::new(0, config.url, config.max_connections as u32)
            .map(|c| AccountingPool(Arc::new(c)))
            .map_err(AccountingError::Connect)
    }

    async fn get(&self) -> Result<Self::Connection, Self::Error> {
        Ok(Arc::clone(&self.0))
    }

    async fn close(&self) {}
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub(crate) struct List<T> {
    #[serde(default, skip_serializing_if = "is_zero")]
    #[schemars(example = "example_count")]
    pub count: usize,
    pub items: Vec<T>,
}

fn example_count() -> usize {
    1
}

fn is_zero(num: &usize) -> bool {
    *num == 0
}

impl<T> From<Vec<T>> for List<T> {
    fn from(value: Vec<T>) -> Self {
        Self {
            count: value.len(),
            items: value,
        }
    }
}
