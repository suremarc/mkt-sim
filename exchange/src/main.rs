use exchange::{accounts, assets, auth, Accounting, MetaConn};
use rocket::{fairing::AdHoc, launch};
use rocket_db_pools::Database;

#[launch]
fn rocket() -> _ {
    rocket::build()
        .attach(MetaConn::fairing())
        .attach(Accounting::init())
        .attach(AdHoc::try_on_ignite(
            "create admin account",
            accounts::create_admin_user,
        ))
        .mount("/assets/", assets::routes())
        .mount("/accounts", accounts::routes())
        .mount("/auth", auth::routes())
}
