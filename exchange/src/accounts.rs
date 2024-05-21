use bitflags::bitflags;
use diesel::{
    backend::Backend,
    deserialize::{self, FromSql},
    serialize::{self, Output, ToSql},
    sql_types::BigInt,
    QueryDsl, RunQueryDsl,
};
use rocket::{get, http::Status, post, routes, serde::json::Json, Route};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};

use crate::{
    schema::users::dsl,
    types::{Email, Password, Uuid},
    MetaConn,
};

use diesel::{
    deserialize::FromSqlRow, expression::AsExpression, prelude::Insertable, Queryable, Selectable,
};

use super::List;

pub fn routes() -> Vec<Route> {
    routes![register, get_account_by_id, list_accounts]
}

#[derive(Debug, Clone, Serialize, Deserialize, Queryable, Selectable, Insertable)]
#[diesel(table_name = crate::schema::users)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct User {
    pub id: Uuid,
    pub email: Email,
    #[serde(skip_serializing)]
    pub password: Password,
    #[serde(rename = "roles")]
    pub role_flags: Roles,
}

bitflags! {
    #[derive(Debug, Clone, Copy, Serialize, Deserialize, FromSqlRow, AsExpression, Hash, Eq, PartialEq)]
    #[repr(transparent)]
    #[diesel(sql_type = BigInt)]
    pub struct Roles: i64 {
        const ADMIN = 1 << 0;
        const USER = 1 << 1;
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

#[get("/<id>")]
async fn get_account_by_id(conn: MetaConn, id: uuid::Uuid) -> Result<Json<User>, Status> {
    conn.run(move |c| dsl::users.find(Uuid(id)).first(c))
        .await
        .map(Json)
        .map_err(|e| match e {
            diesel::result::Error::NotFound => Status::NotFound,
            _ => Status::InternalServerError,
        })
}

#[get("/")]
async fn list_accounts(conn: MetaConn) -> Result<Json<List<User>>, Status> {
    conn.run(|c| dsl::users.load(c))
        .await
        .map(List::from)
        .map(Json)
        .map_err(|_e| Status::InternalServerError)
}

#[derive(Deserialize)]
struct NewAccountForm {
    email: Email,
    password: SecretString,
    #[serde(default)]
    admin: bool,
}

#[post("/", data = "<form>")]
async fn register(conn: MetaConn, form: Json<NewAccountForm>) -> Result<Json<User>, Status> {
    let form = form.0;

    let id = Uuid(uuid::Uuid::new_v4());
    let hash = bcrypt::hash(form.password.expose_secret(), bcrypt::DEFAULT_COST)
        .map(SecretString::new)
        .map(Password)
        .map_err(|_e| Status::InternalServerError)?;

    let mut role_flags = Roles::USER;
    role_flags.set(Roles::ADMIN, form.admin);

    conn.run(move |c| {
        diesel::insert_into(dsl::users)
            .values(User {
                id,
                email: form.email,
                password: hash,
                role_flags,
            })
            .get_result(c)
    })
    .await
    .map(Json)
    .map_err(|e| match e {
        diesel::result::Error::DatabaseError(_, _) => Status::Conflict,
        _ => Status::InternalServerError,
    })
}
