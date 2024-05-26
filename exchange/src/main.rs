use exchange::{accounts, assets, auth, Accounting, MetaConn};
use rocket::{fairing::AdHoc, launch};
use rocket_db_pools::Database;
use rocket_okapi::{
    settings::UrlObject,
    swagger_ui::{make_swagger_ui, SwaggerUIConfig},
};

#[launch]
fn rocket() -> _ {
    tracing_subscriber::fmt::init();

    rocket::build()
        .attach(MetaConn::fairing())
        .attach(Accounting::init())
        .attach(AdHoc::try_on_ignite(
            "create admin account",
            accounts::create_admin_user,
        ))
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
