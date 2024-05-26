use std::ops::Deref;

use chrono::{DateTime, Utc};
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
use jsonwebtoken as jwt;
use rocket::{
    http::Status,
    post,
    request::{FromRequest, Outcome},
    serde::json::Json,
    Request, Route,
};
use rocket_okapi::{
    gen::OpenApiGenerator,
    okapi::openapi3::{SecurityRequirement, SecurityScheme, SecuritySchemeData},
    openapi, openapi_get_routes,
    request::{OpenApiFromRequest, RequestHeaderInput},
};
use schemars::JsonSchema;
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};

use crate::{
    accounts::{Roles, User},
    types::{Email, Password, Uuid},
    MetaConn,
};

pub fn routes() -> Vec<Route> {
    openapi_get_routes![login]
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct LoginForm {
    email: Email,
    password: Password,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AuthnClaim {
    pub id: Uuid,
    pub roles: Roles,
    pub expires: DateTime<Utc>,
}

#[openapi]
#[post("/session", data = "<form>")]
async fn login(
    conn: MetaConn,
    form: Json<LoginForm>,
    jwt_secret: JwtSecretKey,
) -> Result<String, Status> {
    use crate::schema::users::dsl;

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
        roles: user.role_flags,
        expires: chrono::offset::Utc::now() + chrono::Days::new(7),
    };

    jwt::encode(
        &jwt::Header::default(),
        &claim,
        &jwt::EncodingKey::from_secret(jwt_secret.expose_secret().as_bytes()),
    )
    .map_err(|_e| Status::InternalServerError)
}

pub struct JwtSecretKey(SecretString);

impl Deref for JwtSecretKey {
    type Target = SecretString;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[async_trait::async_trait]
impl<'r> FromRequest<'r> for JwtSecretKey {
    type Error = ();

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        match req.rocket().figment().extract_inner("jwt.token") {
            Err(_e) => Outcome::Error((Status::InternalServerError, ())),
            Ok(r) => Outcome::Success(JwtSecretKey(r)),
        }
    }
}

impl<'r> OpenApiFromRequest<'r> for JwtSecretKey {
    fn from_request_input(
        _gen: &mut OpenApiGenerator,
        _name: String,
        _required: bool,
    ) -> rocket_okapi::Result<RequestHeaderInput> {
        let mut requirements = SecurityRequirement::new();
        requirements.insert("token".to_string(), vec![]);
        Ok(RequestHeaderInput::Security(
            "token".to_string(),
            SecurityScheme {
                description: None,
                data: SecuritySchemeData::Http {
                    scheme: "Bearer".to_string(),
                    bearer_format: None,
                },
                extensions: Default::default(),
            },
            requirements,
        ))
    }
}

#[async_trait::async_trait]
impl<'r> FromRequest<'r> for AuthnClaim {
    type Error = ();

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        req.guard::<JwtSecretKey>()
            .await
            .and_then(|jwt_secret| {
                if let Some(token) = req
                    .headers()
                    .get_one("Authorization")
                    .and_then(|auth_header| auth_header.strip_prefix("Bearer "))
                {
                    let decoding_key =
                        jwt::DecodingKey::from_secret(jwt_secret.expose_secret().as_bytes());
                    let validation = jwt::Validation::default();

                    match jwt::decode::<AuthnClaim>(token, &decoding_key, &validation) {
                        Ok(data) => Outcome::Success(data.claims),
                        Err(_) => Outcome::Error((Status::Unauthorized, ())),
                    }
                } else {
                    Outcome::Error((Status::Unauthorized, ()))
                }
            })
            .and_then(|claim| {
                if claim.expires < chrono::offset::Utc::now() {
                    Outcome::Error((Status::Unauthorized, ()))
                } else {
                    Outcome::Success(claim)
                }
            })
    }
}

pub struct RoleCheck<const FLAGS: i64>(pub AuthnClaim);

impl<'r, const FLAGS: i64> OpenApiFromRequest<'r> for RoleCheck<FLAGS> {
    fn from_request_input(
        gen: &mut OpenApiGenerator,
        name: String,
        required: bool,
    ) -> rocket_okapi::Result<RequestHeaderInput> {
        JwtSecretKey::from_request_input(gen, name, required)
    }
}

const ADMIN: i64 = Roles::ADMIN.bits();
const USER: i64 = Roles::USER.bits();

pub type AdminCheck = RoleCheck<ADMIN>;
pub type UserCheck = RoleCheck<USER>;

#[async_trait::async_trait]
impl<'r, const FLAGS: i64> FromRequest<'r> for RoleCheck<FLAGS> {
    type Error = ();

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let roles = Roles::from_bits_truncate(FLAGS);

        req.guard::<AuthnClaim>().await.and_then(|claim| {
            if !claim.roles.contains(roles) {
                Outcome::Error((Status::Unauthorized, ()))
            } else {
                Outcome::Success(Self(claim))
            }
        })
    }
}
