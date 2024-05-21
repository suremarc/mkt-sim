use exchange::{accounts, assets, auth, Accounting, MetaConn};
use rocket::launch;
use rocket_db_pools::Database;

#[launch]
fn rocket() -> _ {
    rocket::build()
        .attach(MetaConn::fairing())
        .attach(Accounting::init())
        .mount("/assets/", assets::routes())
        .mount("/accounts", accounts::routes())
        .mount("/auth", auth::routes())
}
