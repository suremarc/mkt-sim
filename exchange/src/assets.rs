use diesel::{prelude::Insertable, QueryDsl, Queryable, RunQueryDsl, Selectable};
use rocket::{get, http::Status, post, routes, serde::json::Json, Route};
use serde::{Deserialize, Serialize};

use crate::{schema::equities::dsl::equities, MetaConn};

use super::List;

#[derive(Debug, Clone, Serialize, Deserialize, Queryable, Selectable, Insertable)]
#[diesel(table_name = crate::schema::equities)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Equity {
    pub ticker: String,
    pub description: Option<String>,
}

pub fn routes() -> Vec<Route> {
    routes![create_equity, get_equity_by_ticker, list_equities]
}

#[get("/equities/<ticker>")]
async fn get_equity_by_ticker(conn: MetaConn, ticker: String) -> Result<Json<Equity>, Status> {
    conn.run(move |c| equities.find(ticker).first(c))
        .await
        .map(Json)
        .map_err(|e| match e {
            diesel::result::Error::NotFound => Status::NotFound,
            _ => Status::InternalServerError,
        })
}

#[get("/equities")]
async fn list_equities(conn: MetaConn) -> Result<Json<List<Equity>>, Status> {
    conn.run(|c| equities.get_results(c))
        .await
        .map(List::from)
        .map(Json)
        .map_err(|_e| Status::InternalServerError)
}

#[post("/equities", data = "<form>")]
async fn create_equity(conn: MetaConn, form: Json<Equity>) -> Result<Json<Equity>, Status> {
    conn.run(move |c| diesel::insert_into(equities).values(form.0).get_result(c))
        .await
        .map(Json)
        .map_err(|e| match e {
            diesel::result::Error::DatabaseError(_, _) => Status::Conflict,
            _ => Status::InternalServerError,
        })
}
