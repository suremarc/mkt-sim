use chrono::NaiveDate;
use rocket::{get, http::Status, post, routes, serde::json::Json, Route};
use rocket_db_pools::Connection;
use serde::{Deserialize, Serialize};
use sqlx::prelude::FromRow;

use crate::Metadata;

use super::List;

pub fn routes() -> Vec<Route> {
    routes![get_equity, list_equities, create_equity]
}

#[derive(Debug, Clone, Deserialize, Serialize, FromRow)]
struct Equity {
    ticker: String,
    description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EquityOption {
    underlying: String,
    expiration_date: NaiveDate,
    contract_type: ContractType,
    strike_price: u32,
    exercise_style: ExerciseStyle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[repr(u8)]
pub enum ContractType {
    Call = b'C',
    Put = b'P',
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExerciseStyle {
    American,
    European,
}

#[get("/equities/<ticker>")]
async fn get_equity(mut db: Connection<Metadata>, ticker: &str) -> Result<Json<Equity>, Status> {
    sqlx::query_as!(Equity, "SELECT * FROM equities WHERE ticker = ?", ticker)
        .fetch_one(&mut **db)
        .await
        .map(Json)
        .map_err(|_e| Status::NotFound)
}

#[get("/equities")]
async fn list_equities(mut db: Connection<Metadata>) -> Result<Json<List<Equity>>, Status> {
    // todo: pagination
    sqlx::query_as!(Equity, "SELECT * FROM equities")
        .fetch_all(&mut **db)
        .await
        .map(List::from)
        .map(Json)
        .map_err(|_e| Status::NotFound)
}

#[post("/equities", data = "<form>")]
async fn create_equity(mut db: Connection<Metadata>, form: Json<Equity>) -> Result<(), Status> {
    sqlx::query!(
        "INSERT INTO equities VALUES (?, ?)",
        form.ticker,
        form.description
    )
    .execute(&mut **db)
    .await
    .map_err(|_e| Status::Conflict)?;

    Ok(())
}
