//! [`Kind`], [`Source`] and [`Outcome`] in SQLite, as the text serde names
//! them.

use crate::host::{Kind, Outcome, Source};
use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef};
use serde::{
    Deserialize,
    de::value::{Error, StrDeserializer},
};

impl ToSql for Kind {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(self.to_string().into())
    }
}

impl FromSql for Kind {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        named(value)
    }
}

impl ToSql for Source {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(self.to_string().into())
    }
}

impl FromSql for Source {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        named(value)
    }
}

impl ToSql for Outcome {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(self.to_string().into())
    }
}

impl FromSql for Outcome {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        named(value)
    }
}

/// The value serde names as `value`'s text.
fn named<'de, T: Deserialize<'de>>(value: ValueRef<'_>) -> FromSqlResult<T> {
    T::deserialize(StrDeserializer::<Error>::new(value.as_str()?))
        .map_err(|error| FromSqlError::Other(error.into()))
}
