use std::{ops::Deref, sync::Arc, time::Duration};

use deadpool::Runtime;
use diesel::{ConnectionError, ConnectionResult};
use figment::Figment;
use hickory_resolver::{error::ResolveError, TokioAsyncResolver};
use itertools::Itertools;
use rocket::futures::{future::BoxFuture, FutureExt};
use rocket_db_pools::{
    diesel::{
        pooled_connection::{AsyncDieselConnectionManager, ManagerConfig},
        AsyncPgConnection,
    },
    Config, Database, Pool,
};
use rustls::pki_types::CertificateDer;
use tigerbeetle_unofficial as tb;

pub mod api;
pub mod process;

#[derive(Database)]
#[database("meta")]
pub struct Meta(pub PgPool);

#[derive(Database)]
#[database("orders")]
pub struct Orders(pub rocket_db_pools::deadpool_redis::Pool);

#[derive(Database)]
#[database("accounting")]
pub struct Accounting(pub AccountingPool);

pub struct PgPool(pub rocket_db_pools::diesel::PgPool);

#[async_trait::async_trait]
impl Pool for PgPool {
    type Connection = <rocket_db_pools::diesel::PgPool as Pool>::Connection;

    type Error = <rocket_db_pools::diesel::PgPool as Pool>::Error;

    async fn init(figment: &Figment) -> Result<Self, Self::Error> {
        let config: Config = figment.extract()?;
        let mut manager_config = ManagerConfig::default();
        manager_config.custom_setup = Box::new(establish_connection);
        let manager =
            AsyncDieselConnectionManager::new_with_config(config.url.as_str(), manager_config);

        deadpool::managed::Pool::builder(manager)
            .max_size(config.max_connections)
            .wait_timeout(Some(Duration::from_secs(config.connect_timeout)))
            .create_timeout(Some(Duration::from_secs(config.connect_timeout)))
            .recycle_timeout(config.idle_timeout.map(Duration::from_secs))
            .runtime(Runtime::Tokio1)
            .build()
            .map(Self)
            .map_err(rocket_db_pools::Error::Init)
    }

    async fn get(&self) -> Result<Self::Connection, Self::Error> {
        <rocket_db_pools::diesel::PgPool as Pool>::get(&self.0).await
    }

    async fn close(&self) {
        self.0.close()
    }
}

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

fn establish_connection(config: &str) -> BoxFuture<ConnectionResult<AsyncPgConnection>> {
    let fut = async {
        // We first set up the way we want rustls to work.
        let rustls_config = rustls::ClientConfig::builder()
            .with_root_certificates(root_certs())
            .with_no_client_auth();
        let tls = tokio_postgres_rustls::MakeRustlsConnect::new(rustls_config);
        let (client, conn) = tokio_postgres::connect(config, tls)
            .await
            .map_err(|e| ConnectionError::BadConnection(e.to_string()))?;
        tokio::spawn(async move {
            if let Err(e) = conn.await {
                tracing::error!("Database connection: {e}");
            }
        });
        AsyncPgConnection::try_from(client).await
    };

    fut.boxed()
}

fn root_certs() -> rustls::RootCertStore {
    let mut roots = rustls::RootCertStore::empty();
    let certs = rustls_native_certs::load_native_certs().expect("Certs not loadable!");
    roots.add_parsable_certificates(certs);
    roots.add_parsable_certificates(Some(CertificateDer::from(
        include_bytes!("ca-certificate.crt").as_slice(),
    )));
    roots
}
