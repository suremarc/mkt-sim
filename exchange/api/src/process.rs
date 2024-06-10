use rocket::{Build, Rocket};
use rocket_db_pools::Database;

use crate::{Accounting, Orders};

pub fn rocket() -> Rocket<Build> {
    rocket::build()
        .attach(Accounting::init())
        .attach(Orders::init())
}
