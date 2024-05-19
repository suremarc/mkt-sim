use rocket::{get, http::Status, post, routes, serde::json::Json, Build, Rocket};
use rocket_db_pools::{Connection, Database};
use serde::{Deserialize, Serialize};
use sqlx::prelude::FromRow;

use crate::{Accounting, Instruments};

pub fn rocket() -> Rocket<Build> {
    rocket::build()
        .attach(Instruments::init())
        .attach(Accounting::init())
        .mount(
            "/",
            routes![get_instruments_equity, create_instruments_equity],
        )
}

#[get("/instruments/equities/<ticker>", format = "json")]
async fn get_instruments_equity(
    mut instruments: Connection<Instruments>,
    ticker: &str,
) -> Result<Json<InstrumentEquity>, Status> {
    let row = sqlx::query_as!(
        InstrumentEquity,
        "SELECT * FROM equities WHERE ticker = ?",
        ticker
    )
    .fetch_one(&mut **instruments)
    .await
    .map_err(|_e| Status::NotFound)?;

    Ok(Json(row))
}

#[derive(Debug, Clone, Deserialize, Serialize, FromRow)]
struct InstrumentEquity {
    ticker: String,
    description: Option<String>,
}

#[post("/instruments/equities", data = "<form>")]
async fn create_instruments_equity(
    mut instruments: Connection<Instruments>,
    form: Json<InstrumentEquity>,
) -> Result<(), (Status, &'static str)> {
    sqlx::query!(
        "INSERT INTO equities VALUES (?, ?)",
        form.ticker,
        form.description
    )
    .execute(&mut **instruments)
    .await
    .map_err(|_e| (Status::Conflict, "record already exists"))?;

    Ok(())
}
