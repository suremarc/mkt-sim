use std::str::FromStr;

use chrono::NaiveDate;
use diesel::{
    backend::Backend,
    deserialize::{FromSql, FromSqlRow},
    expression::AsExpression,
    prelude::Insertable,
    serialize::{Output, ToSql},
    sql_types::{Integer, Text},
    ExpressionMethods, QueryDsl, Queryable, RunQueryDsl, Selectable,
};
use itertools::Itertools;
use rocket::{get, http::Status, post, routes, serde::json::Json, Route};
use serde::{Deserialize, Serialize};
use strum_macros::{EnumString, IntoStaticStr};

use crate::MetaConn;

use super::List;

pub fn routes() -> Vec<Route> {
    routes![
        create_equity,
        get_equity_by_id,
        get_equity_by_ticker,
        list_equities,
        create_equity_option,
        list_equity_options_by_underlying_ticker,
        list_equity_options_by_underlying_id
    ]
}

#[derive(Debug, Clone, Serialize, Deserialize, Queryable, Selectable)]
#[diesel(table_name = crate::schema::equities)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Equity {
    pub id: i32,
    pub ticker: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Queryable, Selectable, Insertable)]
#[diesel(table_name = crate::schema::equity_options)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct EquityOption {
    pub underlying: i32,
    pub expiration_date: NaiveDate,
    pub contract_type: ContractType,
    pub strike_price: Mills,
    pub exercise_style: ExerciseStyle,
}

#[derive(
    Debug, Clone, Copy, Deserialize, Serialize, FromSqlRow, AsExpression, EnumString, IntoStaticStr,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
#[diesel(sql_type = Text)]
pub enum ContractType {
    Call,
    Put,
}

impl<B: Backend> FromSql<Text, B> for ContractType
where
    String: FromSql<Text, B>,
{
    fn from_sql(bytes: <B as Backend>::RawValue<'_>) -> diesel::deserialize::Result<Self> {
        String::from_sql(bytes).and_then(|v| {
            Self::from_str(&v).map_err(|e| format!("invalid contract type: {e}").into())
        })
    }
}

impl<B: Backend> ToSql<Text, B> for ContractType
where
    str: ToSql<Text, B>,
{
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, B>) -> diesel::serialize::Result {
        str::to_sql(self.into(), out)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, FromSqlRow, AsExpression)]
#[serde(transparent)]
#[diesel(sql_type = Integer)]
pub struct Mills(pub i32);

impl<B: Backend> FromSql<Integer, B> for Mills
where
    i32: FromSql<Integer, B>,
{
    fn from_sql(bytes: <B as Backend>::RawValue<'_>) -> diesel::deserialize::Result<Self> {
        i32::from_sql(bytes).map(Self)
    }
}

impl<B: Backend> ToSql<Integer, B> for Mills
where
    i32: ToSql<Integer, B>,
{
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, B>) -> diesel::serialize::Result {
        i32::to_sql(&self.0, out)
    }
}

#[derive(
    Debug, Clone, Copy, Deserialize, Serialize, FromSqlRow, AsExpression, EnumString, IntoStaticStr,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
#[diesel(sql_type = Text)]
pub enum ExerciseStyle {
    American,
    European,
}

impl<B: Backend> FromSql<Text, B> for ExerciseStyle
where
    String: FromSql<Text, B>,
{
    fn from_sql(bytes: <B as Backend>::RawValue<'_>) -> diesel::deserialize::Result<Self> {
        String::from_sql(bytes).and_then(|v| {
            Self::from_str(&v).map_err(|e| format!("invalid contract type: {e}").into())
        })
    }
}

impl<B: Backend> ToSql<Text, B> for ExerciseStyle
where
    str: ToSql<Text, B>,
{
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, B>) -> diesel::serialize::Result {
        str::to_sql(self.into(), out)
    }
}

#[get("/equities/<ticker>", rank = 1)]
async fn get_equity_by_ticker(conn: MetaConn, ticker: String) -> Result<Json<Equity>, Status> {
    use crate::schema::equities::dsl;
    conn.run(move |c| dsl::equities.filter(dsl::ticker.eq(ticker)).first(c))
        .await
        .map(Json)
        .map_err(|e| match e {
            diesel::result::Error::NotFound => Status::NotFound,
            _ => Status::InternalServerError,
        })
}

#[get("/equities/<id>", rank = 0)]
async fn get_equity_by_id(conn: MetaConn, id: i32) -> Result<Json<Equity>, Status> {
    conn.run(move |c| crate::schema::equities::dsl::equities.find(id).first(c))
        .await
        .map(Json)
        .map_err(|e| match e {
            diesel::result::Error::NotFound => Status::NotFound,
            _ => Status::InternalServerError,
        })
}

#[get("/equities")]
async fn list_equities(conn: MetaConn) -> Result<Json<List<Equity>>, Status> {
    conn.run(|c| crate::schema::equities::dsl::equities.get_results(c))
        .await
        .map(List::from)
        .map(Json)
        .map_err(|_e| Status::InternalServerError)
}

#[derive(Debug, Clone, Deserialize, Insertable)]
#[diesel(table_name = crate::schema::equities)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
struct CreateEquityForm {
    pub ticker: String,
    pub description: Option<String>,
}

#[post("/equities", data = "<form>")]
async fn create_equity(
    conn: MetaConn,
    form: Json<CreateEquityForm>,
) -> Result<Json<Equity>, Status> {
    conn.run(move |c| {
        diesel::insert_into(crate::schema::equities::dsl::equities)
            .values(form.0)
            .get_result(c)
    })
    .await
    .map(Json)
    .map_err(|e| match e {
        diesel::result::Error::DatabaseError(_, _) => Status::Conflict,
        _ => Status::InternalServerError,
    })
}

#[derive(Debug, Clone, Serialize)]
struct EquityOptionResponse {
    pub ticker: String,
    #[serde(flatten)]
    pub option: EquityOption,
}

#[post("/equity-options", data = "<form>")]
async fn create_equity_option(
    conn: MetaConn,
    form: Json<EquityOption>,
) -> Result<Json<EquityOptionResponse>, Status> {
    let underlying = form.underlying;

    let option = conn
        .run(move |c| {
            use crate::schema::equity_options::dsl::*;
            diesel::insert_into(equity_options)
                .values(&form.0)
                .get_result(c)
        })
        .await
        .map_err(|e| match e {
            diesel::result::Error::DatabaseError(_, _) => Status::Conflict,
            _ => Status::InternalServerError,
        })?;

    let ticker = conn
        .run(move |c| {
            use crate::schema::equities::dsl::*;
            equities.select(ticker).find(underlying).first(c)
        })
        .await
        .map_err(|e| match e {
            diesel::result::Error::NotFound => Status::NotFound,
            _ => Status::InternalServerError,
        })?;

    Ok(Json(EquityOptionResponse { ticker, option }))
}

#[get("/equity-options/<ticker>", rank = 1)]
async fn list_equity_options_by_underlying_ticker(
    conn: MetaConn,
    ticker: String,
) -> Result<Json<List<EquityOptionResponse>>, Status> {
    let results: Vec<(EquityOption, Equity)> = conn
        .run(move |c| {
            use crate::schema::equities::dsl;

            crate::schema::equity_options::dsl::equity_options
                .inner_join(dsl::equities)
                .filter(dsl::ticker.eq(ticker))
                .load(c)
        })
        .await
        .map_err(|e| match e {
            diesel::result::Error::NotFound => Status::NotFound,
            _ => Status::InternalServerError,
        })?;

    Ok(Json(List::from(
        results
            .into_iter()
            .map(|(option, underlying)| EquityOptionResponse {
                ticker: underlying.ticker,
                option,
            })
            .collect_vec(),
    )))
}

#[get("/equity-options/<id>", rank = 0)]
async fn list_equity_options_by_underlying_id(
    conn: MetaConn,
    id: i32,
) -> Result<Json<List<EquityOptionResponse>>, Status> {
    let results: Vec<(EquityOption, Equity)> = conn
        .run(move |c| {
            use crate::schema::equity_options::dsl::*;
            equity_options
                .inner_join(crate::schema::equities::dsl::equities)
                .filter(underlying.eq(id))
                .load(c)
        })
        .await
        .map_err(|e| match e {
            diesel::result::Error::NotFound => Status::NotFound,
            _ => Status::InternalServerError,
        })?;

    Ok(Json(List::from(
        results
            .into_iter()
            .map(|(option, underlying)| EquityOptionResponse {
                ticker: underlying.ticker,
                option,
            })
            .collect_vec(),
    )))
}
