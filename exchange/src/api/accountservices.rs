use diesel::{QueryDsl, RunQueryDsl};
use rocket::{get, http::Status, routes, serde::json::Json, Route};

use crate::{
    models::{User, Uuid},
    schema::users::dsl::users,
    AccountServicesConn,
};

pub fn routes() -> Vec<Route> {
    routes![get_account_by_id]
}

#[get("/accounts/<id>")]
async fn get_account_by_id(
    conn: AccountServicesConn,
    id: uuid::Uuid,
) -> Result<Json<User>, Status> {
    conn.run(move |c| users.find(Uuid(id)).first::<User>(c))
        .await
        .map(Json)
        .map_err(|e| match e {
            diesel::result::Error::NotFound => Status::NotFound,
            _ => Status::InternalServerError,
        })
}
