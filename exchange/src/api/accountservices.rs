use diesel::{QueryDsl, RunQueryDsl};
use rocket::{get, http::Status, post, routes, serde::json::Json, Route};
use serde::{Deserialize, Serialize};

use crate::{schema::users::dsl::users, MetaConn};

use std::{
    fmt::{self, Display, Formatter},
    str::FromStr,
};

use diesel::{
    backend::Backend,
    deserialize::{self, FromSql, FromSqlRow},
    expression::AsExpression,
    prelude::Insertable,
    serialize::{self, Output, ToSql},
    sql_types::Binary,
    Queryable, Selectable,
};
use email_address::EmailAddress;

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
    pub password: String,
    pub role_flags: i64,
}

pub enum Role {
    Admin = 1 << 0,
    User = 1 << 1,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromSqlRow, AsExpression, Hash, Eq, PartialEq)]
#[serde(transparent)]
#[diesel(sql_type = Binary)]
pub struct Email(EmailAddress);

impl TryFrom<String> for Email {
    type Error = email_address::Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        EmailAddress::from_str(&value).map(Self)
    }
}

impl From<Email> for String {
    fn from(value: Email) -> Self {
        value.0.into()
    }
}

#[derive(
    Debug,
    Default,
    Clone,
    Copy,
    Serialize,
    Deserialize,
    FromSqlRow,
    AsExpression,
    Hash,
    Eq,
    PartialEq,
)]
#[serde(transparent)]
#[diesel(sql_type = Binary)]
pub struct Uuid(pub uuid::Uuid);

impl Uuid {
    pub fn new_v4() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

impl From<Uuid> for uuid::Uuid {
    fn from(s: Uuid) -> Self {
        s.0
    }
}

impl From<uuid::Uuid> for Uuid {
    fn from(s: uuid::Uuid) -> Self {
        Uuid(s)
    }
}

impl Display for Uuid {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl<B: Backend> FromSql<Binary, B> for Uuid
where
    Vec<u8>: FromSql<Binary, B>,
{
    fn from_sql(bytes: <B as Backend>::RawValue<'_>) -> deserialize::Result<Self> {
        let value = <Vec<u8>>::from_sql(bytes)?;
        uuid::Uuid::from_slice(&value)
            .map(Uuid)
            .map_err(|e| e.into())
    }
}

impl<B: Backend> ToSql<Binary, B> for Uuid
where
    [u8]: ToSql<Binary, B>,
{
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, B>) -> serialize::Result {
        self.0.as_bytes().to_sql(out)
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
    admin: bool,
}

#[post("/register", data = "<form>")]
async fn register(conn: MetaConn, form: Json<NewAccountForm>) -> Result<Json<User>, Status> {
    let id = Uuid(uuid::Uuid::new_v4());
    let hash = bcrypt::hash(&form.password, bcrypt::DEFAULT_COST)
        .map_err(|_e| Status::InternalServerError)?;

    conn.run(move |c| {
        diesel::insert_into(users)
            .values(User {
                id,
                email: form.email.clone(),
                password: hash,
                role_flags: Role::User as i64 | (form.admin as i64 & Role::Admin as i64),
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
