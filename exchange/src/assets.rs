use std::str::FromStr;

use chrono::{NaiveDate, NaiveDateTime};
use diesel::{
    backend::Backend,
    deserialize::{FromSql, FromSqlRow},
    expression::AsExpression,
    prelude::Insertable,
    serialize::{Output, ToSql},
    sql_function,
    sql_types::{Integer, Text},
    Connection, ExpressionMethods, QueryDsl, Queryable, RunQueryDsl, Selectable,
};
use rocket::{get, http::Status, post, serde::json::Json, Route};
use rocket_okapi::{openapi, openapi_get_routes};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum_macros::{EnumString, IntoStaticStr};

use crate::{
    auth::{AdminCheck, UserCheck},
    MetaConn,
};

use super::List;

pub fn routes() -> Vec<Route> {
    openapi_get_routes![
        create_equities,
        get_equity_by_id,
        get_equity_by_ticker,
        list_equities,
        create_equity_options,
        list_equity_options_by_underlying_ticker,
        list_equity_options_by_underlying_id
    ]
}

#[derive(Debug, Clone, Serialize, Deserialize, Queryable, Selectable, JsonSchema)]
#[diesel(table_name = crate::schema::equities)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Equity {
    pub id: i32,
    pub created: NaiveDateTime,
    pub ticker: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Queryable, Selectable, Insertable, JsonSchema)]
#[diesel(table_name = crate::schema::equity_options)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct EquityOption {
    pub underlying: i32,
    pub expiration_date: NaiveDate,
    pub contract_type: ContractType,
    pub strike_price: Mills,
    pub exercise_style: ExerciseStyle,
    pub created: NaiveDateTime,
}

#[derive(
    Debug,
    Clone,
    Copy,
    Deserialize,
    Serialize,
    FromSqlRow,
    AsExpression,
    EnumString,
    IntoStaticStr,
    JsonSchema,
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, FromSqlRow, AsExpression, JsonSchema)]
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
    Debug,
    Clone,
    Copy,
    Deserialize,
    Serialize,
    FromSqlRow,
    AsExpression,
    EnumString,
    IntoStaticStr,
    JsonSchema,
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

/// # Get Equities
///
/// List details for all equity assets.
#[openapi]
#[get("/equities")]
async fn list_equities(_check: UserCheck, conn: MetaConn) -> Result<Json<List<Equity>>, Status> {
    conn.run(|c| crate::schema::equities::dsl::equities.get_results(c))
        .await
        .map(List::from)
        .map(Json)
        .map_err(|_e| Status::InternalServerError)
}

/// # Get Equity
///
/// Get details for an equity asset.
#[openapi]
#[get("/equities/<id>", rank = 0)]
async fn get_equity_by_id(
    _check: UserCheck,
    conn: MetaConn,
    id: i32,
) -> Result<Json<Equity>, Status> {
    conn.run(move |c| crate::schema::equities::dsl::equities.find(id).first(c))
        .await
        .map(Json)
        .map_err(|e| match e {
            diesel::result::Error::NotFound => Status::NotFound,
            _ => Status::InternalServerError,
        })
}

/// # Get Equity by Ticker
///
/// Get details for an equity asset with the given ticker.
#[openapi]
#[get("/equities/<ticker>", rank = 1)]
async fn get_equity_by_ticker(
    _check: UserCheck,
    conn: MetaConn,
    ticker: String,
) -> Result<Json<Equity>, Status> {
    use crate::schema::equities::dsl;
    conn.run(move |c| dsl::equities.filter(dsl::ticker.eq(ticker)).first(c))
        .await
        .map(Json)
        .map_err(|e| match e {
            diesel::result::Error::NotFound => Status::NotFound,
            _ => Status::InternalServerError,
        })
}

#[derive(Debug, Clone, Deserialize, Insertable, JsonSchema)]
#[diesel(table_name = crate::schema::equities)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
struct CreateEquityForm {
    pub ticker: String,
    pub description: Option<String>,
}

sql_function!(fn last_insert_rowid() -> Integer);

/// # Create Equities
///
/// Batch endpoint for registering equities.
#[openapi]
#[post("/equities", data = "<form>")]
async fn create_equities(
    _check: AdminCheck,
    conn: MetaConn,
    form: Json<List<CreateEquityForm>>,
) -> Result<Json<List<Equity>>, Status> {
    conn.run(move |c| {
        use crate::schema::equities::dsl::*;

        c.transaction(|c| {
            let n = form.items.len();
            diesel::insert_into(equities)
                .values(form.0.items)
                .execute(c)?;
            equities
                .order(id.desc())
                .limit(n as i64)
                .order(id.asc())
                .get_results(c)
        })
    })
    .await
    .map(List::from)
    .map(Json)
    .map_err(|e| match e {
        diesel::result::Error::DatabaseError(_, _) => Status::Conflict,
        _ => Status::InternalServerError,
    })
}

/// # Create Equity Options
///
/// Batch endpoint for creating equity options.
/// The underlying equity asset must be registered already.
#[openapi]
#[post("/equities/options", data = "<form>")]
async fn create_equity_options(
    _check: AdminCheck,
    conn: MetaConn,
    form: Json<List<EquityOption>>,
) -> Result<(), Status> {
    conn.run(move |c| {
        use crate::schema::equity_options::dsl::*;
        diesel::insert_into(equity_options)
            .values(&form.items)
            .execute(c)
    })
    .await
    .map_err(|e| match e {
        diesel::result::Error::DatabaseError(_, _) => Status::Conflict,
        _ => Status::InternalServerError,
    })?;

    Ok(())
}

/// # List Equity Options by Underlying
///
/// List all equity options derived from a given underlying equity asset.
#[openapi]
#[get("/equities/<id>/options", rank = 0)]
async fn list_equity_options_by_underlying_id(
    _check: UserCheck,
    conn: MetaConn,
    id: i32,
) -> Result<Json<List<EquityOption>>, Status> {
    conn.run(move |c| {
        use crate::schema::equity_options::dsl::*;
        equity_options
            .filter(underlying.eq(id))
            .select((
                underlying,
                expiration_date,
                contract_type,
                strike_price,
                exercise_style,
                created,
            ))
            .load(c)
    })
    .await
    .map(List::from)
    .map(Json)
    .map_err(|e| match e {
        diesel::result::Error::NotFound => Status::NotFound,
        _ => Status::InternalServerError,
    })
}

/// # List Equity Options By Underlying Ticker
///
/// List all equity options derived from an underlying equity asset with the given ticker.
#[openapi]
#[get("/equities/<ticker>/options", rank = 1)]
async fn list_equity_options_by_underlying_ticker(
    _check: UserCheck,
    conn: MetaConn,
    ticker: String,
) -> Result<Json<List<EquityOption>>, Status> {
    conn.run(move |c| {
        use crate::schema::{
            equities::dsl::{self as equities_dsl, equities},
            equity_options::dsl::*,
        };

        crate::schema::equity_options::dsl::equity_options
            .inner_join(equities)
            .filter(equities_dsl::ticker.eq(ticker))
            .select((
                underlying,
                expiration_date,
                contract_type,
                strike_price,
                exercise_style,
                created,
            ))
            .load(c)
    })
    .await
    .map(List::from)
    .map(Json)
    .map_err(|e| match e {
        diesel::result::Error::NotFound => Status::NotFound,
        _ => Status::InternalServerError,
    })
}
