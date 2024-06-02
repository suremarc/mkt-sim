use std::fmt::Display;

use bitflags::bitflags;
use diesel::{
    backend::Backend,
    deserialize::{self, FromSql},
    result::DatabaseErrorKind,
    serialize::{self, Output, ToSql},
    sql_types::BigInt,
    upsert::excluded,
    ExpressionMethods, QueryDsl, RunQueryDsl,
};
use itertools::Itertools;
use redis::ToRedisArgs;
use redis_derive::{FromRedisValue, ToRedisArgs};
use rocket::{
    fairing, get,
    http::Status,
    post,
    request::{FromParam, FromRequest, Outcome},
    serde::json::Json,
    Build, Request, Rocket,
};
use rocket_db_pools::{deadpool_redis::redis, Connection, Database, Pool};
use rocket_okapi::{
    gen::OpenApiGenerator,
    openapi,
    request::{OpenApiFromRequest, RequestHeaderInput},
};
use schemars::{
    gen::SchemaGenerator,
    schema::{InstanceType, Metadata, SchemaObject},
    JsonSchema,
};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use tigerbeetle_unofficial::{
    self as tb,
    error::{CreateAccountErrorKind, CreateAccountsError},
};
use tracing::error;

use super::{
    auth::{AdminCheck, AuthnClaim, RoleCheck, UserCheck},
    schema::users::dsl,
    types::{Email, Password, Uuid},
    CursorList, ADMIN_ACCOUNT_ID,
};

use crate::{Accounting, MetaConn, Orders};

use diesel::{
    deserialize::FromSqlRow, expression::AsExpression, prelude::Insertable, Queryable, Selectable,
};

use super::List;

#[derive(Debug, Clone, Serialize, Deserialize, Queryable, Selectable, Insertable, JsonSchema)]
#[diesel(table_name = super::schema::users)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct User {
    /// A unique user identifier.
    pub id: Uuid,
    pub email: Email,
    #[serde(skip_serializing)]
    pub password: Password,
    /// A string representation of the user's roles.
    #[serde(rename = "roles")]
    pub role_flags: Roles,
}

bitflags! {
    #[derive(Debug, Clone, Copy, Serialize, Deserialize, FromSqlRow, AsExpression, Hash, Eq, PartialEq)]
    #[repr(transparent)]
    #[diesel(sql_type = BigInt)]
    pub struct Roles: i64 {
        const ADMIN = 1 << 0;
    }
}

impl JsonSchema for Roles {
    fn schema_name() -> String {
        "Roles".to_string()
    }

    fn json_schema(_gen: &mut SchemaGenerator) -> schemars::schema::Schema {
        schemars::schema::Schema::Object(SchemaObject {
            metadata: Some(Box::new(Metadata {
                description: Some("A string representation of a set of roles.".to_string()),
                ..Default::default()
            })),
            instance_type: Some(InstanceType::String.into()),
            ..Default::default()
        })
    }
}

impl<B: Backend> FromSql<BigInt, B> for Roles
where
    i64: FromSql<BigInt, B>,
{
    fn from_sql(bytes: <B as Backend>::RawValue<'_>) -> deserialize::Result<Self> {
        i64::from_sql(bytes).map(Self::from_bits_truncate)
    }
}

impl<B: Backend> ToSql<BigInt, B> for Roles
where
    i64: ToSql<BigInt, B>,
{
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, B>) -> serialize::Result {
        i64::to_sql(self.0.as_ref(), out)
    }
}

/// # Get account
///
/// Fetches a single account by ID.
#[openapi(tag = "Accounts")]
#[get("/accounts/<id>")]
pub async fn get_account_by_id(
    _check: UserIdCheck,
    conn: MetaConn,
    id: Uuid,
) -> Result<Json<User>, Status> {
    conn.run(move |c| dsl::users.find(id).first(c))
        .await
        .map(Json)
        .map_err(|e| match e {
            diesel::result::Error::NotFound => Status::NotFound,
            e => {
                error!("error fetching user from db: {e}");
                Status::InternalServerError
            }
        })
}

/// # List all accounts
#[openapi(tag = "Accounts")]
#[get("/accounts")]
pub async fn list_accounts(_check: AdminCheck, conn: MetaConn) -> Result<Json<List<User>>, Status> {
    conn.run(|c| dsl::users.load(c))
        .await
        .map(List::from)
        .map(Json)
        .map_err(|e| {
            error!("error listing accounts: {e}");
            Status::InternalServerError
        })
}

#[derive(Deserialize, JsonSchema)]
pub struct NewAccountForm {
    email: Email,
    password: Password,
}

/// # Create Account
///
/// Create a new account.
#[openapi(tag = "Accounts")]
#[post("/accounts", data = "<form>")]
pub async fn register(
    conn: MetaConn,
    accounting: Connection<Accounting>,
    form: Json<NewAccountForm>,
) -> Result<Json<User>, Status> {
    let form = form.0;

    let account_id = Uuid(uuid::Uuid::new_v4());
    let hash = bcrypt::hash(form.password.expose_secret(), bcrypt::DEFAULT_COST)
        .map(SecretString::new)
        .map(Password)
        .map_err(|e| {
            error!("error hashing password: {e}");
            Status::InternalServerError
        })?;

    let account = conn
        .run(move |c| {
            diesel::insert_into(dsl::users)
                .values(User {
                    id: account_id,
                    email: form.email,
                    password: hash,
                    role_flags: Roles::empty(),
                })
                .get_result(c)
        })
        .await
        .map(Json)
        .map_err(|e| match e {
            diesel::result::Error::DatabaseError(DatabaseErrorKind::UniqueViolation, _) => {
                Status::Conflict
            }
            e => {
                error!("error inserting user into DB: {e}");
                Status::InternalServerError
            }
        })?;

    match accounting
        .create_accounts(vec![tb::Account::new(account_id.0.as_u128(), u32::MAX, 1)
            .with_user_data_128(account_id.0.as_u128())
            .with_flags(tb::account::Flags::CREDITS_MUST_NOT_EXCEED_DEBITS)])
        .await
    {
        Err(CreateAccountsError::Api(errs))
            if matches!(errs.as_slice()[0].kind(), CreateAccountErrorKind::Exists) => {}
        Ok(()) => {}
        Err(e) => {
            error!("error creating funds account: {e}");
            return Err(Status::InternalServerError);
        }
    }
    // todo: rollback if this fails

    Ok(account)
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct Holdings {
    asset_id: i32,
    amount: i128,
}

/// List equity holdings for an account.
#[openapi(tag = "Accounts")]
#[get("/accounts/<id>/equities")]
pub async fn get_equities_for_account(
    _check: UserIdCheck,
    id: Uuid,
    meta: MetaConn,
    accounting: Connection<Accounting>,
) -> Result<Json<List<Holdings>>, Status> {
    let asset_ids: Vec<i32> = meta
        .run(|c| {
            use super::schema::equities::dsl::*;
            equities.select(asset_id).load(c)
        })
        .await
        .map_err(|e| {
            error!("error fetching equities: {e}");
            Status::InternalServerError
        })?;

    let tb_ids = asset_ids
        .into_iter()
        .map(|asset_id| uuid::Uuid::new_v5(&id.0, &i32::to_be_bytes(asset_id)).as_u128())
        .collect_vec();

    let assets: Vec<tb::Account> = accounting.lookup_accounts(tb_ids).await.map_err(|e| {
        error!("error fetching assets from tigerbeetle: {e}");
        Status::InternalServerError
    })?;

    Ok(Json(List::from(
        assets
            .into_iter()
            .map(|asset| Holdings {
                asset_id: asset.ledger() as i32,
                amount: asset.credits_posted() as i128 - asset.debits_posted() as i128,
            })
            .collect_vec(),
    )))
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToRedisArgs, FromRedisValue)]
pub struct OrderBookEntry {
    pub account_id: Uuid,
    pub asset_id: i32,
    pub price: i32,
    pub size: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(tag = "order_type")]
pub enum OrderType {
    Market,
    Limit { price: i32 },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, ToRedisArgs, FromRedisValue)]
#[serde(rename_all = "snake_case")]
#[redis(rename_all = "snake_case")]
pub enum Book {
    Bids,
    Offers,
}

impl<'a> FromParam<'a> for Book {
    type Error = Box<dyn std::error::Error + Send + Sync>;

    fn from_param(param: &'a str) -> Result<Self, Self::Error> {
        match param {
            "bids" => Ok(Book::Bids),
            "offers" => Ok(Book::Offers),
            _ => Err(format!("invalid book: {param}").into()),
        }
    }
}

impl Display for Book {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bids => write!(f, "bids"),
            Self::Offers => write!(f, "offers"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CreateOrderForm {
    pub size: u32,
    #[serde(flatten)]
    pub order_type: OrderType,
}

impl ToRedisArgs for CreateOrderForm {
    fn write_redis_args<W>(&self, out: &mut W)
    where
        W: ?Sized + redis::RedisWrite,
    {
        self.size.write_redis_args(out);
        self.order_type.write_redis_args(out);
    }
}

impl ToRedisArgs for OrderType {
    fn write_redis_args<W>(&self, out: &mut W)
    where
        W: ?Sized + redis::RedisWrite,
    {
        match self {
            OrderType::Market => "market".write_redis_args(out),
            OrderType::Limit { price } => {
                "limit".write_redis_args(out);
                price.write_redis_args(out);
            }
        }
    }
}

/// Submit an order for an equity asset.
#[openapi(tag = "Accounts")]
#[post("/accounts/<account_id>/assets/<asset_id>/<book>", data = "<form>")]
pub async fn submit_orders_for_account(
    _check: UserIdCheck,
    account_id: Uuid,
    asset_id: i32,
    book: Book,
    mut orders: Connection<Orders>,
    accounting: Connection<Accounting>,
    form: Json<CreateOrderForm>,
) -> Result<String, Status> {
    let order_id = Uuid(uuid::Uuid::now_v7());

    let asset_account_id = uuid::Uuid::new_v5(&account_id.0, &asset_id.to_be_bytes()).as_u128();

    // reserve funds/assets
    // todo: how do you handle market buy orders??
    match book {
        Book::Bids => {
            // they want to buy, so reserve funds
            if let OrderType::Limit { price } = form.order_type {
                // TODO: we should technically handle negative prices here...
                accounting
                    .create_transfers(vec![tb::Transfer::new(order_id.0.as_u128())
                        .with_code(1)
                        .with_amount(form.size as u128 * price as u128)
                        .with_ledger(u32::MAX)
                        .with_credit_account_id(account_id.0.as_u128())
                        .with_debit_account_id(ADMIN_ACCOUNT_ID.0.as_u128())
                        .with_flags(tb::transfer::Flags::PENDING)])
                    .await
                    .map_err(|e| {
                        error!("error reserving funds: {e:?}");
                        Status::InternalServerError
                    })?;
            }
        }
        Book::Offers => {
            // they want to sell, so reserve units of the asset
            match accounting
                .create_accounts(vec![tb::Account::new(asset_account_id, asset_id as u32, 1)
                    .with_user_data_128(account_id.0.as_u128())
                    .with_user_data_32(asset_id as u32)
                    .with_flags(tb::account::Flags::CREDITS_MUST_NOT_EXCEED_DEBITS)])
                .await
            {
                Err(CreateAccountsError::Api(errs))
                    if matches!(errs.as_slice()[0].kind(), CreateAccountErrorKind::Exists) => {}
                Ok(()) => {}
                Err(e) => {
                    error!("error creating asset account: {e:?}");
                    return Err(Status::InternalServerError);
                }
            }
            accounting
                .create_transfers(vec![tb::Transfer::new(order_id.0.as_u128())
                    .with_code(1)
                    .with_amount(form.size as u128)
                    .with_ledger(asset_id as u32)
                    .with_credit_account_id(asset_account_id)
                    .with_debit_account_id(
                        uuid::Uuid::new_v5(&ADMIN_ACCOUNT_ID.0, &asset_id.to_be_bytes()).as_u128(),
                    )
                    .with_flags(tb::transfer::Flags::PENDING)])
                .await
                .map_err(|e| {
                    error!("error reserving assets for sell order: {e}");
                    Status::InternalServerError
                })?;
        }
    }

    let shares_filled: i32 = redis::Script::new(include_str!("scripts/order.lua"))
        .prepare_invoke()
        .key(asset_id)
        .key(format!("{asset_id}_bids"))
        .key(format!("{asset_id}_offers"))
        .key(account_id)
        .key(order_id)
        .arg(book)
        .arg(form.0)
        .invoke_async(orders.as_mut())
        .await
        .map_err(|e| {
            error!("error submitting order: {e}");
            Status::InternalServerError
        })?;

    Ok(format!("{shares_filled}"))
}

#[openapi(tag = "Accounts")]
#[get("/accounts/<id>/assets/orders?<cursor>")]
pub async fn list_orders_for_account(
    _check: UserIdCheck,
    mut orders: Connection<Orders>,
    id: Uuid,
    cursor: Option<usize>,
) -> Result<Json<CursorList<OrderBookEntry>>, Status> {
    let script = redis::Script::new(
        r"
        local cursor, keys = unpack(redis.call('ZSCAN', KEYS[1], ARGV[1]))
        local results = {}
        for i = 1, #keys, 2 do
            local hash = redis.call('HGETALL', keys[i])
            table.insert(results, hash)
        end
        return {cursor, results}
    ",
    );

    script
        .prepare_invoke()
        .key(id)
        .arg(cursor.unwrap_or_default())
        .invoke_async(orders.as_mut())
        .await
        .map(Json)
        .map_err(|e| {
            error!("error listing orders: {e}");
            Status::InternalServerError
        })
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename_all = "snake_case")]
pub enum BalanceTxType {
    Deposit,
    Withdraw,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
pub struct BalanceForm {
    pub amount: u128,
    pub r#type: BalanceTxType,
}

#[openapi(tag = "Accounts")]
#[post("/accounts/<id>/balance", data = "<form>")]
pub async fn deposit_or_withdraw(
    _check: UserIdCheck,
    id: Uuid,
    form: Json<BalanceForm>,
    accounting: Connection<Accounting>,
) -> Result<(), Status> {
    let (debit, credit) = match form.r#type {
        BalanceTxType::Deposit => (id.0.as_u128(), ADMIN_ACCOUNT_ID.0.as_u128()),
        BalanceTxType::Withdraw => (ADMIN_ACCOUNT_ID.0.as_u128(), id.0.as_u128()),
    };

    accounting
        .create_transfers(vec![tb::Transfer::new(uuid::Uuid::now_v7().as_u128())
            .with_code(1)
            .with_amount(form.amount)
            .with_ledger(u32::MAX)
            .with_debit_account_id(debit)
            .with_credit_account_id(credit)])
        .await
        .map_err(|e| {
            error!("error depositing/withdrawing: {e:?}");
            Status::BadRequest
        })?;

    Ok(())
}

#[allow(unused)]
pub struct UserIdCheck(pub AuthnClaim);

#[async_trait::async_trait]
impl<'r> FromRequest<'r> for UserIdCheck {
    type Error = ();

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        if let Outcome::Success(RoleCheck(claim)) = req.guard::<UserCheck>().await {
            // TODO: figure out the relevant parameter dynamically
            if claim.account_id == req.param::<Uuid>(1).unwrap().unwrap() {
                return Outcome::Success(Self(claim));
            }
        } else if let Outcome::Success(RoleCheck(claim)) = req.guard::<AdminCheck>().await {
            return Outcome::Success(Self(claim));
        }

        Outcome::Error((Status::Unauthorized, ()))
    }
}

impl<'r> OpenApiFromRequest<'r> for UserIdCheck {
    fn from_request_input(
        gen: &mut OpenApiGenerator,
        name: String,
        required: bool,
    ) -> rocket_okapi::Result<RequestHeaderInput> {
        AuthnClaim::from_request_input(gen, name, required)
    }
}

pub async fn create_admin_user(rocket: Rocket<Build>) -> fairing::Result {
    let form: NewAccountForm = match rocket.figment().focus("admin").extract() {
        Err(e) => {
            error!("error reading admin config: {e}");
            return Err(rocket);
        }
        Ok(form) => form,
    };

    let hash = match bcrypt::hash(form.password.expose_secret(), bcrypt::DEFAULT_COST)
        .map(SecretString::new)
        .map(Password)
    {
        Err(e) => {
            error!("error hashing admin password: {e}");
            return Err(rocket);
        }
        Ok(hash) => hash,
    };

    let conn = if let Some(conn) = MetaConn::get_one(&rocket).await {
        conn
    } else {
        return Err(rocket);
    };

    let accounting = if let Some(accounting) = Accounting::fetch(&rocket) {
        accounting
    } else {
        return Err(rocket);
    };
    let accounting_conn = match accounting.get().await {
        Err(e) => {
            error!("error getting accounting cxn: {e}");
            return Err(rocket);
        }
        Ok(conn) => conn,
    };

    match accounting_conn
        .create_accounts(vec![tb::Account::new(
            ADMIN_ACCOUNT_ID.0.as_u128(),
            u32::MAX,
            1,
        )
        .with_user_data_128(ADMIN_ACCOUNT_ID.0.as_u128())])
        .await
    {
        Err(CreateAccountsError::Api(errs))
            if matches!(errs.as_slice()[0].kind(), CreateAccountErrorKind::Exists) => {}
        Ok(()) => {}
        Err(e) => {
            error!("error setting up admin funds account: {e:?}");
            return Err(rocket);
        }
    };

    let res = conn
        .run(move |c| {
            use super::schema::users::dsl;

            diesel::insert_into(dsl::users)
                .values(User {
                    id: ADMIN_ACCOUNT_ID,
                    email: form.email,
                    password: hash,
                    role_flags: Roles::ADMIN,
                })
                .on_conflict(dsl::email)
                .do_update()
                .set(dsl::password.eq(excluded(dsl::password)))
                .execute(c)
        })
        .await;

    match res {
        Err(e) => {
            error!("error setting up admin account: {e}");
            Err(rocket)
        }
        Ok(_) => Ok(rocket),
    }
}
