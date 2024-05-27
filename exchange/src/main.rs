use diesel_migrations::MigrationHarness;
use diesel_migrations::{embed_migrations, EmbeddedMigrations};
use exchange::{accounts, assets, auth, Accounting, MetaConn, Orders};
use rocket::{
    fairing::{self, AdHoc},
    launch, Build, Rocket,
};
use rocket_db_pools::Database;
use rocket_okapi::{
    settings::UrlObject,
    swagger_ui::{make_swagger_ui, SwaggerUIConfig},
};
use tracing::error;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

#[launch]
fn rocket() -> _ {
    tracing_subscriber::fmt::init();

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

async fn migrate(rocket: Rocket<Build>) -> fairing::Result {
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
