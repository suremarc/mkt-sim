use diesel_async_migrations::{embed_migrations, EmbeddedMigrations};
use rocket::{
    fairing::{self, AdHoc},
    Build, Rocket,
};
use rocket_db_pools::{
    deadpool_redis::redis::{self, FromRedisValue},
    Database,
};
use rocket_okapi::{
    openapi_get_routes,
    settings::UrlObject,
    swagger_ui::{make_swagger_ui, SwaggerUIConfig},
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tracing::error;

use crate::{Accounting, Meta, Orders};

mod accounts;
mod assets;
mod auth;
#[rustfmt::skip]
pub mod schema;
pub mod types;

pub const ADMIN_ACCOUNT_ID: uuid::Uuid = uuid::Uuid::from_bytes([
    0x6e, 0xb6, 0x21, 0x61, 0x0c, 0xdb, 0x47, 0x1a, 0xaf, 0x31, 0x61, 0x94, 0x9f, 0x09, 0x49, 0x5b,
]);

pub fn rocket() -> Rocket<Build> {
    rocket::build()
        .attach(Meta::init())
        .attach(Accounting::init())
        .attach(Orders::init())
        .attach(AdHoc::try_on_ignite("migrate", migrate))
        .mount(
            "/",
            openapi_get_routes![
                auth::login,
                accounts::register,
                accounts::get_account_by_id,
                accounts::list_accounts,
                accounts::get_equities_for_account,
                accounts::submit_orders_for_account,
                accounts::list_orders_for_account,
                accounts::deposit_or_withdraw,
                assets::create_equities,
                assets::get_equity_by_id,
                assets::get_equity_by_ticker,
                assets::list_equities,
                assets::create_equity_options,
                assets::list_equity_options_by_underlying_id,
                assets::list_equity_options_by_underlying_ticker,
                assets::get_order_book,
            ],
        )
        .mount(
            "/swagger",
            make_swagger_ui(&SwaggerUIConfig {
                urls: vec![UrlObject::new("API", "../openapi.json")],
                ..Default::default()
            }),
        )
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct List<T> {
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

impl<T: FromRedisValue> FromRedisValue for List<T> {
    fn from_redis_value(v: &redis::Value) -> redis::RedisResult<Self> {
        Vec::<T>::from_redis_value(v).map(Self::from)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CursorList<T> {
    #[serde(flatten)]
    inner: List<T>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<usize>,
}

impl<T: FromRedisValue> FromRedisValue for CursorList<T> {
    fn from_redis_value(v: &redis::Value) -> redis::RedisResult<Self> {
        if !v.looks_like_cursor() {
            return Err(redis::RedisError::from((
                redis::ErrorKind::TypeError,
                "expected cursor repsonse",
            )));
        }

        match *v {
            redis::Value::Bulk(ref items) => {
                let cursor = usize::from_redis_value(&items[0])?;
                Ok(Self {
                    inner: List::from_redis_value(&items[1])?,
                    cursor: (cursor > 0).then_some(cursor),
                })
            }
            _ => unreachable!(),
        }
    }
}

async fn migrate(rocket: Rocket<Build>) -> fairing::Result {
    static MIGRATIONS: EmbeddedMigrations = embed_migrations!();

    let meta = match Meta::fetch(&rocket) {
        None => return Err(rocket),
        Some(conn) => conn,
    };

    let mut meta_conn = match meta.get().await {
        Err(e) => {
            error!("error getting meta cxn: {e}");
            return Err(rocket);
        }
        Ok(cxn) => cxn,
    };
    tracing::info!("got meta cxn");

    if let Err(e) = MIGRATIONS.run_pending_migrations(&mut meta_conn).await {
        error!("error performing migrations: {e}");
        return Err(rocket);
    };

    tracing::info!("finished diesel migrations");

    accounts::create_admin_user(rocket).await
}
