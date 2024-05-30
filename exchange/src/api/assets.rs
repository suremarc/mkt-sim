use std::str::FromStr;

use chrono::{NaiveDate, NaiveDateTime};
use diesel::{
    backend::Backend,
    deserialize::{FromSql, FromSqlRow},
    expression::AsExpression,
    prelude::Insertable,
    result::{DatabaseErrorKind, Error::DatabaseError},
    serialize::{Output, ToSql},
    sql_function,
    sql_types::{Integer, Text},
    Connection, ExpressionMethods, QueryDsl, Queryable, RunQueryDsl, Selectable,
};
use rocket::{get, http::Status, post, serde::json::Json};
use rocket_okapi::openapi;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum_macros::{EnumString, IntoStaticStr};
use tracing::error;

use super::auth::{AdminCheck, UserCheck};

use super::List;
use crate::MetaConn;

#[derive(Debug, Clone, Serialize, Deserialize, Queryable, Selectable, JsonSchema)]
#[diesel(table_name = super::schema::equities)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Equity {
    /// A unique identifier for equity assets.
    pub id: i32,
    /// A unique global identifier for this asset.
    pub asset_id: i32,
    /// A common identifier for equity assets, usually five letters or less.
    pub ticker: String,
    /// Description of the company that this asset is derived from.
    pub description: Option<String>,
    /// Date & time of creation in RFC 3339 format.
    pub created: NaiveDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, Queryable, Selectable, JsonSchema)]
#[diesel(table_name = super::schema::equity_options)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct EquityOption {
    /// A unique identifier for equity options.
    pub id: i32,
    /// A unique global identifier for this asset.
    pub asset_id: i32,
    /// ID of the equity asset underlying this option.
    pub underlying: i32,
    /// Date that this contract expires.
    pub expiration_date: NaiveDate,
    /// The kind of contract (call or put).
    pub contract_type: ContractType,
    /// The strike price, measured in mills.
    pub strike_price: Mills,
    /// Date & time of creation in RFC 3339 format.
    pub created: NaiveDateTime,
}

/// # Get Equities
///
/// List details for all equity assets.
#[openapi(tag = "Assets")]
#[get("/assets/equities")]
pub async fn list_equities(
    _check: UserCheck,
    conn: MetaConn,
) -> Result<Json<List<Equity>>, Status> {
    conn.run(|c| super::schema::equities::dsl::equities.get_results(c))
        .await
        .map(List::from)
        .map(Json)
        .map_err(|e| {
            error!("error listing equities: {e}");
            Status::InternalServerError
        })
}

/// # Get Equity
///
/// Get details for an equity asset.
#[openapi(tag = "Assets")]
#[get("/assets/equities/<id>", rank = 0)]
pub async fn get_equity_by_id(
    _check: UserCheck,
    conn: MetaConn,
    id: i32,
) -> Result<Json<Equity>, Status> {
    conn.run(move |c| super::schema::equities::dsl::equities.find(id).first(c))
        .await
        .map(Json)
        .map_err(|e| match e {
            diesel::result::Error::NotFound => Status::NotFound,
            e => {
                error!("error fetching equity from db: {e}");
                Status::InternalServerError
            }
        })
}

/// # Get Equity by Ticker
///
/// Get details for an equity asset with the given ticker.
#[openapi(tag = "Assets")]
#[get("/assets/equities/<ticker>", rank = 1)]
pub async fn get_equity_by_ticker(
    _check: UserCheck,
    conn: MetaConn,
    ticker: String,
) -> Result<Json<Equity>, Status> {
    use super::schema::equities::dsl;
    conn.run(move |c| dsl::equities.filter(dsl::ticker.eq(ticker)).first(c))
        .await
        .map(Json)
        .map_err(|e| match e {
            diesel::result::Error::NotFound => Status::NotFound,
            e => {
                error!("error fetching equity from db: {e}");
                Status::InternalServerError
            }
        })
}

#[derive(Debug, Clone, Deserialize, Insertable, JsonSchema)]
#[diesel(table_name = super::schema::equities)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct CreateEquityForm {
    pub ticker: String,
    pub description: Option<String>,
}

sql_function!(fn last_insert_rowid() -> Integer);

/// # Create Equities
///
/// Batch endpoint for registering equities.
#[openapi(tag = "Assets")]
#[post("/assets/equities", data = "<form>")]
pub async fn create_equities(
    _check: AdminCheck,
    conn: MetaConn,
    form: Json<List<CreateEquityForm>>,
) -> Result<Json<List<Equity>>, Status> {
    conn.run(move |c| {
        use super::schema::equities::dsl::*;

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
        DatabaseError(DatabaseErrorKind::UniqueViolation, _) => Status::Conflict,
        e => {
            error!("error creating equities: {e}");
            Status::InternalServerError
        }
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, Insertable, JsonSchema)]
#[diesel(table_name = super::schema::equity_options)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct CreateEquityOptionItem {
    /// ID of the equity asset underlying this option.
    pub underlying: i32,
    /// Date that this contract expires.
    pub expiration_date: NaiveDate,
    /// The kind of contract (call or put).
    pub contract_type: ContractType,
    /// The strike price, measured in mills.
    pub strike_price: Mills,
}

/// # Create Equity Options
///
/// Batch endpoint for creating equity options.
/// The underlying equity asset must be registered already.
#[openapi(tag = "Assets")]
#[post("/assets/equities/options", data = "<form>")]
pub async fn create_equity_options(
    _check: AdminCheck,
    conn: MetaConn,
    form: Json<List<CreateEquityOptionItem>>,
) -> Result<Json<List<EquityOption>>, Status> {
    conn.run(move |c| {
        c.transaction(|c| {
            let n = form.items.len();
            use super::schema::equity_options::dsl::*;
            diesel::insert_into(equity_options)
                .values(&form.items)
                .execute(c)?;
            equity_options
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
        DatabaseError(DatabaseErrorKind::UniqueViolation, _) => Status::Conflict,
        DatabaseError(DatabaseErrorKind::ForeignKeyViolation, _) => Status::UnprocessableEntity,
        e => {
            error!("error creating equity options: {e}");
            Status::InternalServerError
        }
    })
}

/// # List Equity Options by Underlying
///
/// List all equity options derived from a given underlying equity asset.
#[openapi(tag = "Assets")]
#[get("/assets/equities/<id>/options", rank = 0)]
pub async fn list_equity_options_by_underlying_id(
    _check: UserCheck,
    conn: MetaConn,
    id: i32,
) -> Result<Json<List<EquityOption>>, Status> {
    let underlying_id = id;
    conn.run(move |c| {
        use super::schema::equity_options::dsl::*;
        equity_options
            .filter(underlying.eq(underlying_id))
            .select((
                id,
                asset_id,
                underlying,
                expiration_date,
                contract_type,
                strike_price,
                created,
            ))
            .load(c)
    })
    .await
    .map(List::from)
    .map(Json)
    .map_err(|e| match e {
        diesel::result::Error::NotFound => Status::NotFound,
        e => {
            error!("error listing equity options: {e}");
            Status::InternalServerError
        }
    })
}

/// # List Equity Options By Underlying Ticker
///
/// List all equity options derived from an underlying equity asset with the given ticker.
#[openapi(tag = "Assets")]
#[get("/assets/equities/<ticker>/options", rank = 1)]
pub async fn list_equity_options_by_underlying_ticker(
    _check: UserCheck,
    conn: MetaConn,
    ticker: String,
) -> Result<Json<List<EquityOption>>, Status> {
    conn.run(move |c| {
        use super::schema::{
            equities::dsl::{self as equities_dsl, equities},
            equity_options::dsl::*,
        };

        super::schema::equity_options::dsl::equity_options
            .inner_join(equities)
            .filter(equities_dsl::ticker.eq(ticker))
            .select((
                id,
                asset_id,
                underlying,
                expiration_date,
                contract_type,
                strike_price,
                created,
            ))
            .load(c)
    })
    .await
    .map(List::from)
    .map(Json)
    .map_err(|e| match e {
        diesel::result::Error::NotFound => Status::NotFound,
        e => {
            error!("error listing equity options: {e}");
            Status::InternalServerError
        }
    })
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
    /// Confers the right (but not the obligation) to buy the underlying asset at the strike price.
    Call,
    /// Confers the right (but not the obligation) to sell the underlying asset at the strike price.
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
