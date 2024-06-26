use std::ops::Deref;

use diesel::ExpressionMethods;
use jsonwebtoken as jwt;
use rocket::{
    http::Status,
    outcome::{try_outcome, IntoOutcome},
    post,
    request::{FromRequest, Outcome},
    serde::json::Json,
    Request,
};
use rocket_db_pools::{
    diesel::prelude::{QueryDsl, RunQueryDsl},
    Connection,
};
use rocket_okapi::{
    gen::OpenApiGenerator,
    okapi::openapi3::{SecurityRequirement, SecurityScheme, SecuritySchemeData},
    openapi,
    request::{OpenApiFromRequest, RequestHeaderInput},
};
use schemars::JsonSchema;
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use tracing::error;

use super::{
    accounts::{Roles, User},
    types::{Email, Password},
};

use crate::Meta;

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct LoginForm {
    email: Email,
    password: Password,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct AuthnClaim {
    pub sub: uuid::Uuid,
    pub exp: u64,

    pub roles: Roles,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct AuthResponse {
    pub claim: AuthnClaim,
    pub token: String,
}
/// # Login
///
/// Returns an auth bearer token that lasts a week from its creation.
#[openapi(tag = "Auth")]
#[post("/auth/token", data = "<form>")]
pub async fn login(
    mut conn: Connection<Meta>,
    form: Json<LoginForm>,
    jwt_secret: JwtSecretKey,
) -> Result<Json<AuthResponse>, Status> {
    use super::schema::users::dsl;

    let email = form.email.clone();
    let user: User = dsl::users
        .filter(dsl::email.eq(&email))
        .first(&mut conn)
        .await
        .map_err(|e| match e {
            diesel::result::Error::NotFound => Status::NotFound,
            e => {
                error!("error fetching user from db: {e}");
                Status::InternalServerError
            }
        })?;

    // check password
    let password_check =
        bcrypt::verify(form.password.expose_secret(), user.password.expose_secret()).map_err(
            |e| {
                error!("error verifying password: {e}");
                Status::InternalServerError
            },
        )?;
    if !password_check {
        return Err(Status::NotFound);
    }

    let claim = AuthnClaim {
        sub: user.id,
        exp: (chrono::offset::Utc::now() + chrono::Days::new(7)).timestamp() as u64,
        roles: user.role_flags,
    };

    jwt::encode(
        &jwt::Header::default(),
        &claim,
        &jwt::EncodingKey::from_secret(jwt_secret.expose_secret().as_bytes()),
    )
    .map(|token| Json(AuthResponse { claim, token }))
    .map_err(|e| {
        error!("error encoding jwt: {e}");
        Status::InternalServerError
    })
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
        req.rocket()
            .figment()
            .extract_inner("jwt.secret")
            .map(JwtSecretKey)
            .map_err(|e| error!("error reading jwt secret key: {e}"))
            .or_error(Status::InternalServerError)
    }
}

impl<'r> OpenApiFromRequest<'r> for JwtSecretKey {
    fn from_request_input(
        _gen: &mut OpenApiGenerator,
        _name: String,
        _required: bool,
    ) -> rocket_okapi::Result<RequestHeaderInput> {
        Ok(RequestHeaderInput::None)
    }
}

#[async_trait::async_trait]
impl<'r> FromRequest<'r> for AuthnClaim {
    type Error = ();

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        req.guard::<JwtSecretKey>().await.and_then(|jwt_secret| {
            if let Some(token) = req
                .headers()
                .get_one("Authorization")
                .and_then(|auth_header| auth_header.strip_prefix("Bearer "))
            {
                let decoding_key =
                    jwt::DecodingKey::from_secret(jwt_secret.expose_secret().as_bytes());
                let validation = jwt::Validation::default();

                jwt::decode::<AuthnClaim>(token, &decoding_key, &validation)
                    .map(|token| token.claims)
                    .map_err(|e| {
                        error!("error decoding jwt: {e}");
                    })
                    .or_error(Status::Unauthorized)
            } else {
                Outcome::Error((Status::Unauthorized, ()))
            }
        })
    }
}

impl<'r> OpenApiFromRequest<'r> for AuthnClaim {
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

pub struct RoleCheck<const FLAGS: i64>(pub AuthnClaim);

impl<'r, const FLAGS: i64> OpenApiFromRequest<'r> for RoleCheck<FLAGS> {
    fn from_request_input(
        gen: &mut OpenApiGenerator,
        name: String,
        required: bool,
    ) -> rocket_okapi::Result<RequestHeaderInput> {
        AuthnClaim::from_request_input(gen, name, required)
    }
}

const ADMIN: i64 = Roles::ADMIN.bits();

pub type AdminCheck = RoleCheck<ADMIN>;
pub type UserCheck = RoleCheck<0>;

#[async_trait::async_trait]
impl<'r, const FLAGS: i64> FromRequest<'r> for RoleCheck<FLAGS> {
    type Error = ();

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let required_roles = Roles::from_bits_truncate(FLAGS);
        let claim = try_outcome!(req.guard::<AuthnClaim>().await);

        if !claim.roles.contains(required_roles) {
            Outcome::Error((Status::Forbidden, ()))
        } else {
            Outcome::Success(Self(claim))
        }
    }
}
