use bitflags::bitflags;
use diesel::{sql_types::BigInt, QueryDsl, RunQueryDsl};
use rocket::{get, http::Status, post, routes, serde::json::Json, Route};
use serde::{Deserialize, Serialize};

use crate::{
    schema::users::dsl::users,
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
    #[diesel(serialize_as = String, deserialize_as = String)]
    pub email: Email,
    #[serde(skip_serializing)]
    #[diesel(serialize_as = String, deserialize_as = String)]
    pub password: Password,
    #[diesel(serialize_as = i64, deserialize_as = i64)]
    #[serde(rename = "roles")]
    pub role_flags: Roles,
}

bitflags! {
    #[derive(Debug, Clone, Serialize, Deserialize, FromSqlRow, AsExpression, Hash, Eq, PartialEq)]
    #[repr(transparent)]
    #[diesel(sql_type = BigInt)]
    pub struct Roles: i64 {
        const ADMIN = 1 << 0;
        const USER = 1 << 1;
    }
}

impl From<i64> for Roles {
    fn from(value: i64) -> Self {
        Self::from_bits_truncate(value)
    }
}

impl From<Roles> for i64 {
    fn from(value: Roles) -> Self {
        value.bits()
    }
}

#[get("/accounts/<id>")]
async fn get_account_by_id(conn: MetaConn, id: uuid::Uuid) -> Result<Json<User>, Status> {
    conn.run(move |c| users.find(Uuid(id)).first::<User>(c))
        .await
        .map(Json)
        .map_err(|e| match e {
            diesel::result::Error::NotFound => Status::NotFound,
            _ => Status::InternalServerError,
        })
}

#[get("/accounts")]
async fn list_accounts(conn: MetaConn) -> Result<Json<List<User>>, Status> {
    conn.run(|c| users.get_results(c))
        .await
        .map(List::from)
        .map(Json)
        .map_err(|_e| Status::InternalServerError)
}

#[derive(Serialize, Deserialize)]
struct NewAccountForm {
    email: Email,
    password: String,
    #[serde(default)]
    admin: bool,
}

#[post("/register", data = "<form>")]
async fn register(conn: MetaConn, form: Json<NewAccountForm>) -> Result<Json<User>, Status> {
    let form = form.0;

    let id = Uuid(uuid::Uuid::new_v4());
    let hash = bcrypt::hash(&form.password, bcrypt::DEFAULT_COST)
        .map_err(|_e| Status::InternalServerError)?;

    let mut role_flags = Roles::USER;
    role_flags.set(Roles::ADMIN, form.admin);

    conn.run(move |c| {
        diesel::insert_into(users)
            .values(User {
                id,
                email: form.email,
                password: hash.into(),
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
