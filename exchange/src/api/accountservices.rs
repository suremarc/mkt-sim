use std::collections::BTreeMap;

use email_address::EmailAddress;
use rocket::{get, http::Status, post, routes, serde::json::Json, Route};
use rocket_db_pools::Connection;
use serde::{Deserialize, Serialize};
use sqlx::prelude::Connection as SqlxConnection;
use sqlx::{prelude::Type, types::Uuid};
use sqlx::{Sqlite, Transaction};

use crate::Metadata;

pub fn routes() -> Vec<Route> {
    routes![register, get_account_by_id]
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
#[sqlx(rename_all = "snake_case")]
enum Role {
    Admin,
    User,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
#[sqlx(rename_all = "snake_case")]
enum Scope {
    Create,
    Read,
    Update,
    Delete,
}

#[derive(Debug, Clone, Serialize)]
struct GetUserResult {
    id: Uuid,
    email: EmailAddress,
    roles: Vec<Role>,
    permissions: BTreeMap<Scope, bool>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct RegistrationForm {
    email: String,
    password: String,
    admin: bool,
}

#[get("/accounts/<id>")]
async fn get_account_by_id(
    mut db: Connection<Metadata>,
    id: Uuid,
) -> Result<Json<GetUserResult>, Status> {
    let mut tx = db.begin().await.map_err(|_e| Status::InternalServerError)?;

    get_account_by_id_internal(&mut tx, id).await
}

async fn get_account_by_id_internal(
    tx: &mut Transaction<'_, Sqlite>,
    id: Uuid,
) -> Result<Json<GetUserResult>, Status> {
    let email = sqlx::query_scalar!("SELECT email FROM users WHERE id = ?", id)
        .fetch_one(&mut **tx)
        .await
        .map_err(|_e| Status::InternalServerError)?;

    let permissions = sqlx::query!(
        r#"SELECT 
            scope AS "scope!: Scope", 
            max(admin) AS "admin!: bool"
        FROM role_permissions 
        NATURAL INNER JOIN user_roles
        WHERE user_id = ?
        GROUP BY scope
        "#,
        id,
    )
    .fetch_all(&mut **tx)
    .await
    .map_err(|_e| Status::InternalServerError)?
    .into_iter()
    .map(|r| (r.scope, r.admin))
    .collect();

    Ok(Json(GetUserResult {
        id,
        email: EmailAddress::new_unchecked(email),
        roles: vec![],
        permissions,
    }))
}

#[post("/register", data = "<form>")]
async fn register(
    mut db: Connection<Metadata>,
    form: Json<RegistrationForm>,
) -> Result<Json<GetUserResult>, Status> {
    let id = Uuid::new_v4();
    let hashed = bcrypt::hash(form.password.clone(), bcrypt::DEFAULT_COST)
        .map_err(|_e| Status::InternalServerError)?;

    let mut tx = db.begin().await.map_err(|_e| Status::InternalServerError)?;
    sqlx::query!("INSERT INTO users VALUES (?, ?, ?)", id, form.email, hashed)
        .execute(&mut *tx)
        .await
        .map_err(|_e| Status::Conflict)?;

    sqlx::query!("INSERT INTO user_roles VALUES (?, ?)", id, Role::User,)
        .execute(&mut *tx)
        .await
        .map_err(|_e| Status::InternalServerError)?;

    let result = get_account_by_id_internal(&mut tx, id).await?;

    tx.commit()
        .await
        .map_err(|_e| Status::InternalServerError)?;

    Ok(result)
}
