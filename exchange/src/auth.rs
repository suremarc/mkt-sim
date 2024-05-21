use chrono::{DateTime, Utc};
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
use jsonwebtoken as jwt;
use rocket::{
    http::Status,
    post,
    request::{FromRequest, Outcome},
    routes,
    serde::json::Json,
    Request, Route,
};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};

use crate::{
    accounts::{Roles, User},
    types::{Email, Uuid},
    JwtSecretKey, MetaConn,
};

pub fn routes() -> Vec<Route> {
    routes![login]
}

#[derive(Debug, Clone, Deserialize)]
struct LoginForm {
    email: Email,
    password: SecretString,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AuthnClaim {
    pub id: Uuid,
    pub roles: Roles,
    pub expires: DateTime<Utc>,
}

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
