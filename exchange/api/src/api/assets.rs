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
    ExpressionMethods, Queryable, Selectable,
};
use redis::FromRedisValue;
use rocket::{get, http::Status, post, serde::json::Json};
use rocket_db_pools::{
    diesel::prelude::{QueryDsl, RunQueryDsl},
    Connection,
};
use rocket_okapi::openapi;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum_macros::{EnumString, IntoStaticStr};
use tracing::error;

use super::{
    accounts::Book,
    auth::{AdminCheck, UserCheck},
    CursorList,
};

use super::List;
use crate::{Meta, Orders};

#[derive(Debug, Clone, Serialize, Deserialize, Queryable, Selectable, JsonSchema)]
#[diesel(table_name = super::schema::equities)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Equity {
    /// A unique global identifier for this asset.
    pub id: i32,
    /// A common identifier for equity assets, usually five letters or less.
    pub ticker: String,
    /// Description of the company that this asset is derived from.
    pub description: Option<String>,
    /// Date & time of creation in RFC 3339 format.
    pub created: NaiveDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, Queryable, Selectable, JsonSchema)]
#[diesel(table_name = super::schema::equity_options)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct EquityOption {
    /// A unique global identifier for this asset.
    pub id: i32,
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
    mut conn: Connection<Meta>,
) -> Result<Json<List<Equity>>, Status> {
    super::schema::equities::dsl::equities
        .get_results(&mut conn)
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
#[get("/assets/equities/<asset_id>", rank = 0)]
pub async fn get_equity_by_id(
    _check: UserCheck,
    mut conn: Connection<Meta>,
    asset_id: i32,
) -> Result<Json<Equity>, Status> {
    super::schema::equities::dsl::equities
        .find(asset_id)
        .first(&mut conn)
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
    mut conn: Connection<Meta>,
    ticker: String,
) -> Result<Json<Equity>, Status> {
    use super::schema::equities::dsl;
    dsl::equities
        .filter(dsl::ticker.eq(ticker))
        .first(&mut conn)
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
#[diesel(check_for_backend(diesel::pg::Pg))]
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
    mut conn: Connection<Meta>,
    form: Json<List<CreateEquityForm>>,
) -> Result<Json<List<Equity>>, Status> {
    {
        use super::schema::equities::dsl::*;
        diesel::insert_into(equities)
            .values(form.0.items)
            .get_results(&mut conn)
    }
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
#[diesel(check_for_backend(diesel::pg::Pg))]
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
    mut conn: Connection<Meta>,
    form: Json<List<CreateEquityOptionItem>>,
) -> Result<Json<List<EquityOption>>, Status> {
    {
        use super::schema::equity_options::dsl::*;
        diesel::insert_into(equity_options)
            .values(&form.items)
            .get_results(&mut conn)
    }
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
#[get("/assets/equities/<asset_id>/options", rank = 0)]
pub async fn list_equity_options_by_underlying_id(
    _check: UserCheck,
    mut conn: Connection<Meta>,
    asset_id: i32,
) -> Result<Json<List<EquityOption>>, Status> {
    {
        use super::schema::equity_options::dsl::*;
        equity_options
            .filter(underlying.eq(asset_id))
            .load(&mut conn)
    }
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
    mut conn: Connection<Meta>,
    ticker: String,
) -> Result<Json<List<EquityOption>>, Status> {
    {
        use super::schema::{
            equities::dsl::{self as equities_dsl, equities},
            equity_options::dsl::*,
        };

        super::schema::equity_options::dsl::equity_options
            .inner_join(equities)
            .filter(equities_dsl::ticker.eq(ticker))
            .select((
                id,
                underlying,
                expiration_date,
                contract_type,
                strike_price,
                created,
            ))
            .load(&mut conn)
    }
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

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AnonymizedOrderBookEntry {
    pub price: i32,
    pub size: u32,
}

impl FromRedisValue for AnonymizedOrderBookEntry {
    fn from_redis_value(v: &redis::Value) -> redis::RedisResult<Self> {
        <(i32, u32)>::from_redis_value(v).map(|(price, size)| Self { price, size })
    }
}

/// # Get Order Book
///
/// Show the current state of the order book for a given asset.
#[openapi(tag = "Assets")]
#[get("/assets/<asset_id>/<book>?<cursor>", rank = 3)]
pub async fn get_order_book(
    _check: UserCheck,
    asset_id: i32,
    book: Book,
    cursor: Option<usize>,
    mut orders: rocket_db_pools::Connection<Orders>,
) -> Result<Json<CursorList<AnonymizedOrderBookEntry>>, Status> {
    let script = redis::Script::new(
        r"
        local cursor, keys = unpack(redis.call('ZSCAN', KEYS[1], ARGV[1]))
        local results = {}
        for i = 1, #keys, 2 do
            local hash = redis.call('HMGET', keys[i], 'price', 'size')
            table.insert(results, hash)
        end

        return {cursor, results}
    ",
    );

    script
        .prepare_invoke()
        .key(format!("{asset_id}_{book}"))
        .arg(cursor.unwrap_or_default())
        .invoke_async(orders.as_mut())
        .await
        .map(Json)
        .map_err(|e| {
            error!("error listing orders: {e}");
            Status::InternalServerError
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
