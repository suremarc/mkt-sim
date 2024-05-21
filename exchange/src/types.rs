use core::fmt;
use std::{
    fmt::{Display, Formatter},
    str::FromStr,
};

use diesel::{
    backend::Backend,
    deserialize::{self, FromSql, FromSqlRow},
    expression::AsExpression,
    serialize::{self, Output, ToSql},
    sql_types::Binary,
};
use email_address::EmailAddress;
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
#[serde(transparent)]
pub struct Password(SecretString);

impl From<String> for Password {
    fn from(value: String) -> Self {
        Self(SecretString::new(value))
    }
}

impl From<Password> for String {
    fn from(value: Password) -> Self {
        value.0.expose_secret().clone()
    }
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
