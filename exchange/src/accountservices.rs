use bitflags::bitflags;
use chrono::{DateTime, Utc};
use diesel::{
    backend::Backend,
    deserialize::{self, FromSql},
    serialize::{self, Output, ToSql},
    sql_types::BigInt,
    ExpressionMethods, QueryDsl, RunQueryDsl,
};
use jsonwebtoken as jwt;
use jwt::EncodingKey;
use rocket::{
    get,
    http::{Cookie, CookieJar, Status},
    post, routes,
    serde::json::Json,
    Route,
};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};

use crate::{
    schema::users::dsl,
    types::{Email, Password, Uuid},
    JwtSecretKey, MetaConn,
};

use diesel::{
    deserialize::FromSqlRow, expression::AsExpression, prelude::Insertable, Queryable, Selectable,
};

use super::List;

pub fn routes() -> Vec<Route> {
    routes![register, get_account_by_id, list_accounts, login]
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

#[get("/accounts/<id>")]
async fn get_account_by_id(conn: MetaConn, id: uuid::Uuid) -> Result<Json<User>, Status> {
    conn.run(move |c| dsl::users.find(Uuid(id)).first(c))
        .await
        .map(Json)
        .map_err(|e| match e {
            diesel::result::Error::NotFound => Status::NotFound,
            _ => Status::InternalServerError,
        })
}

#[get("/accounts")]
async fn list_accounts(conn: MetaConn) -> Result<Json<List<User>>, Status> {
    conn.run(|c| dsl::users.get_results(c))
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

#[post("/register", data = "<form>")]
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

#[derive(Debug, Clone, Deserialize)]
struct LoginForm {
    email: Email,
    password: SecretString,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct AuthnClaim {
    id: Uuid,
    expires: DateTime<Utc>,
}

#[post("/login", data = "<form>")]
async fn login(
    conn: MetaConn,
    form: Json<LoginForm>,
    jar: &CookieJar<'_>,
    jwt_secret: JwtSecretKey,
) -> Result<(), Status> {
    let email = form.email.clone();
    let user: User = conn
        .run(move |c| dsl::users.filter(dsl::email.eq(&email)).first(c))
        .await
        .map_err(|e| match e {
            diesel::result::Error::NotFound => Status::NotFound,
            _ => Status::InternalServerError,
        })?;

    // check password
    if !bcrypt::verify(form.password.expose_secret(), user.password.expose_secret())
        .map_err(|_e| Status::InternalServerError)?
    {
        return Err(Status::NotFound);
    }

    let claim = AuthnClaim {
        id: user.id,
        expires: chrono::offset::Utc::now() + chrono::Days::new(7),
    };

    let token = jwt::encode(
        &jwt::Header::default(),
        &claim,
        &EncodingKey::from_secret(jwt_secret.expose_secret().as_bytes()),
    )
    .map_err(|_e| Status::InternalServerError)?;

    jar.add(Cookie::new("Authorization", format!("Bearer {token}")));

    Ok(())
}
