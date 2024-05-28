use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use rocket::{
    fairing::{self, AdHoc},
    Build, Rocket,
};
use rocket_db_pools::{
    deadpool_redis::redis::{self, FromRedisValue},
    Database,
};
use rocket_okapi::{
    settings::UrlObject,
    swagger_ui::{make_swagger_ui, SwaggerUIConfig},
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tracing::error;

use crate::{Accounting, MetaConn, Orders};

pub mod accounts;
pub mod assets;
pub mod auth;
#[rustfmt::skip]
pub mod schema;
pub mod orders;
pub mod types;

pub fn rocket() -> Rocket<Build> {
    rocket::build()
        .attach(MetaConn::fairing())
        .attach(Accounting::init())
        .attach(Orders::init())
        .attach(AdHoc::try_on_ignite("migrate", migrate))
        .mount("/assets", assets::routes())
        .mount("/accounts", accounts::routes())
        .mount("/auth", auth::routes())
        .mount(
            "/swagger",
            make_swagger_ui(&SwaggerUIConfig {
                urls: vec![
                    UrlObject::new("Assets", "../assets/openapi.json"),
                    UrlObject::new("Accounts", "../accounts/openapi.json"),
                    UrlObject::new("Authentication", "../auth/openapi.json"),
                ],
                ..Default::default()
            }),
        )
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

impl<T: FromRedisValue> FromRedisValue for List<T> {
    fn from_redis_value(v: &redis::Value) -> redis::RedisResult<Self> {
        Vec::<T>::from_redis_value(v).map(Self::from)
    }
}

async fn migrate(rocket: Rocket<Build>) -> fairing::Result {
    const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

    let meta = match MetaConn::get_one(&rocket).await {
        None => return Err(rocket),
        Some(conn) => conn,
    };

    if let Err(e) = meta
        .run(|c| {
            c.run_pending_migrations(MIGRATIONS)?;
            Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
        })
        .await
    {
        error!("error performing migrations: {e}");
        return Err(rocket);
    };

    accounts::create_admin_user(rocket).await
}
