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
use rocket::{
    fairing, get,
    http::Status,
    post,
    request::{FromRequest, Outcome},
    serde::json::Json,
    Build, Request, Rocket, Route,
};
use rocket_okapi::{
    gen::OpenApiGenerator,
    openapi, openapi_get_routes,
    request::{OpenApiFromRequest, RequestHeaderInput},
};
use schemars::{
    gen::SchemaGenerator,
    schema::{InstanceType, Metadata, SchemaObject},
    JsonSchema,
};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use tracing::error;

use crate::{
    auth::{AdminCheck, AuthnClaim, RoleCheck, UserCheck},
    schema::users::dsl,
    types::{Email, Password, Uuid},
    MetaConn,
};

use diesel::{
    deserialize::FromSqlRow, expression::AsExpression, prelude::Insertable, Queryable, Selectable,
};

use super::List;

pub fn routes() -> Vec<Route> {
    openapi_get_routes![register, get_account_by_id, list_accounts]
}

#[derive(Debug, Clone, Serialize, Deserialize, Queryable, Selectable, Insertable, JsonSchema)]
#[diesel(table_name = crate::schema::users)]
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
        const USER = 1 << 1;
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
#[openapi]
#[get("/<id>")]
async fn get_account_by_id(
    _check: UserIdCheck,
    conn: MetaConn,
    id: uuid::Uuid,
) -> Result<Json<User>, Status> {
    conn.run(move |c| dsl::users.find(Uuid(id)).first(c))
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
#[openapi]
#[get("/")]
async fn list_accounts(_check: AdminCheck, conn: MetaConn) -> Result<Json<List<User>>, Status> {
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
struct NewAccountForm {
    email: Email,
    password: Password,
}

/// # Create Account
///
/// Create a new account.
#[openapi]
#[post("/", data = "<form>")]
async fn register(conn: MetaConn, form: Json<NewAccountForm>) -> Result<Json<User>, Status> {
    let form = form.0;

    let id = Uuid(uuid::Uuid::new_v4());
    let hash = bcrypt::hash(form.password.expose_secret(), bcrypt::DEFAULT_COST)
        .map(SecretString::new)
        .map(Password)
        .map_err(|e| {
            error!("error hashing password: {e}");
            Status::InternalServerError
        })?;

    conn.run(move |c| {
        diesel::insert_into(dsl::users)
            .values(User {
                id,
                email: form.email,
                password: hash,
                role_flags: Roles::USER,
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
    })
}

#[allow(unused)]
struct UserIdCheck(pub AuthnClaim);

#[async_trait::async_trait]
impl<'r> FromRequest<'r> for UserIdCheck {
    type Error = ();

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        if let Outcome::Success(RoleCheck(claim)) = req.guard::<AdminCheck>().await {
            return Outcome::Success(Self(claim));
        } else if let Outcome::Success(RoleCheck(claim)) = req.guard::<UserCheck>().await {
            if claim.id.0 == req.param::<uuid::Uuid>(0).unwrap().unwrap() {
                return Outcome::Success(Self(claim));
            }
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

    let id = Uuid(uuid::Uuid::nil());
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

    let res = conn
        .run(move |c| {
            use crate::schema::users::dsl;

            diesel::insert_into(dsl::users)
                .values(User {
                    id,
                    email: form.email,
                    password: hash,
                    role_flags: Roles::USER | Roles::ADMIN,
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
