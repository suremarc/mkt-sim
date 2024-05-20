use rocket::{Build, Rocket};
use rocket_db_pools::Database;
use serde::{Deserialize, Serialize};

use crate::{AccountServicesConn, Accounting};

mod accountservices;
// mod instruments;

pub fn rocket() -> Rocket<Build> {
    rocket::build()
        .attach(AccountServicesConn::fairing())
        .attach(Accounting::init())
        // .mount("/instruments/", instruments::routes())
        .mount("/accountservices", accountservices::routes())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct List<T> {
    #[serde(default, skip_serializing_if = "is_zero")]
    pub count: usize,
    pub items: Vec<T>,
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
