use core::fmt;
use std::{
    fmt::{Display, Formatter},
    ops::Deref,
    str::FromStr,
};

use diesel::{
    backend::Backend,
    deserialize::{self, FromSql, FromSqlRow},
    expression::AsExpression,
    serialize::{self, Output, ToSql},
    sql_types::{Binary, Text},
};
use email_address::EmailAddress;
use rocket::request::FromParam;
use rocket_db_pools::deadpool_redis::redis::{self, FromRedisValue, ToRedisArgs};
use schemars::{
    gen::SchemaGenerator,
    schema::{InstanceType, SchemaObject},
    JsonSchema,
};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, AsExpression, FromSqlRow)]
#[serde(transparent)]
#[diesel(sql_type = Text)]
pub struct Password(pub SecretString);

impl Deref for Password {
    type Target = SecretString;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<B: Backend> FromSql<Text, B> for Password
where
    String: FromSql<Text, B>,
{
    fn from_sql(bytes: <B as Backend>::RawValue<'_>) -> deserialize::Result<Self> {
        String::from_sql(bytes).map(SecretString::new).map(Self)
    }
}

impl JsonSchema for Password {
    fn schema_name() -> String {
        "password".to_string()
    }

    fn json_schema(_gen: &mut SchemaGenerator) -> schemars::schema::Schema {
        schemars::schema::Schema::Object(SchemaObject {
            instance_type: Some(InstanceType::String.into()),
            format: Some("password".to_string()),
            ..Default::default()
        })
    }
}

impl<B: Backend> ToSql<Text, B> for Password
where
    str: ToSql<Text, B>,
{
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, B>) -> serialize::Result {
        self.0.expose_secret().as_str().to_sql(out)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromSqlRow, AsExpression, Hash, Eq, PartialEq)]
#[serde(transparent)]
#[diesel(sql_type = Text)]
pub struct Email(pub EmailAddress);

impl JsonSchema for Email {
    fn schema_name() -> String {
        "email".to_string()
    }

    fn json_schema(_gen: &mut SchemaGenerator) -> schemars::schema::Schema {
        schemars::schema::Schema::Object(SchemaObject {
            instance_type: Some(InstanceType::String.into()),
            format: Some("email".to_string()),
            ..Default::default()
        })
    }
}

impl Display for Email {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl<B: Backend> FromSql<Text, B> for Email
where
    String: FromSql<Text, B>,
{
    fn from_sql(bytes: <B as Backend>::RawValue<'_>) -> deserialize::Result<Self> {
        let value = String::from_sql(bytes)?;
        EmailAddress::from_str(&value)
            .map(Self)
            .map_err(|e| e.into())
    }
}

impl<B: Backend> ToSql<Text, B> for Email
where
    str: ToSql<Text, B>,
{
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, B>) -> serialize::Result {
        self.0.as_str().to_sql(out)
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
    JsonSchema,
)]
#[serde(transparent)]
#[diesel(sql_type = diesel::sql_types::Uuid)]
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

impl<B: Backend> FromSql<diesel::sql_types::Uuid, B> for Uuid
where
    Vec<u8>: FromSql<diesel::sql_types::Uuid, B>,
{
    fn from_sql(bytes: <B as Backend>::RawValue<'_>) -> deserialize::Result<Self> {
        let value = <Vec<u8>>::from_sql(bytes)?;
        uuid::Uuid::from_slice(&value)
            .map(Uuid)
            .map_err(|e| e.into())
    }
}

impl<B: Backend> ToSql<diesel::sql_types::Uuid, B> for Uuid
where
    [u8]: ToSql<Binary, B>,
{
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, B>) -> serialize::Result {
        self.0.as_bytes().to_sql(out)
    }
}

impl<'a> FromParam<'a> for Uuid {
    type Error = <uuid::Uuid as FromParam<'a>>::Error;

    fn from_param(param: &'a str) -> Result<Self, Self::Error> {
        uuid::Uuid::from_param(param).map(Self)
    }
}

impl FromRedisValue for Uuid {
    fn from_redis_value(v: &redis::Value) -> redis::RedisResult<Self> {
        match *v {
            redis::Value::Data(ref bytes) => uuid::Uuid::from_slice(bytes)
                .map_err(|e| {
                    redis::RedisError::from((
                        redis::ErrorKind::TypeError,
                        "Value is not a valid UUID",
                        e.to_string(),
                    ))
                })
                .map(Self),
            _ => Err(redis::RedisError::from((
                redis::ErrorKind::TypeError,
                "Response was of incompatible type",
                format!(
                    "{:?} (response was {:?})",
                    "Response type not uuid compatible.", v
                ),
            ))),
        }
    }
}

impl ToRedisArgs for Uuid {
    fn write_redis_args<W>(&self, out: &mut W)
    where
        W: ?Sized + redis::RedisWrite,
    {
        out.write_arg(self.0.as_bytes())
    }
}
