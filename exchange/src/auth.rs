use chrono::{DateTime, Utc};
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
use jsonwebtoken as jwt;
use rocket::{
    http::{Cookie, CookieJar, Status},
    post, routes,
    serde::json::Json,
    Route,
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
    id: Uuid,
    admin: bool,
    expires: DateTime<Utc>,
}

#[post("/session", data = "<form>")]
async fn login(
    conn: MetaConn,
    form: Json<LoginForm>,
    jar: &CookieJar<'_>,
    jwt_secret: JwtSecretKey,
) -> Result<(), Status> {
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
        admin: user.role_flags.contains(Roles::ADMIN),
        expires: chrono::offset::Utc::now() + chrono::Days::new(7),
    };

    let token = jwt::encode(
        &jwt::Header::default(),
        &claim,
        &jwt::EncodingKey::from_secret(jwt_secret.expose_secret().as_bytes()),
    )
    .map_err(|_e| Status::InternalServerError)?;

    jar.add(Cookie::new("Authorization", format!("Bearer {token}")));

    Ok(())
}
