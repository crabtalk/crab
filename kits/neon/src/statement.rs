//! One parameterized statement, as Neon's endpoint reads it.

use alloc::{format, string::String};
use berm_lang::{
    Result,
    serde::Serialize,
    serde_json::{self, Value},
};

/// A statement and the parameters bound to its `$n` placeholders.
#[derive(Serialize)]
#[serde(crate = "berm_lang::serde")]
pub struct Statement {
    pub query: String,
    /// A JSON array. Neon binds a string as is, `null` as `NULL`, an array as
    /// a Postgres array, and any other value as its JSON text.
    pub params: Value,
}

impl Statement {
    /// `query` with `params`, which must serialize as a JSON array: a tuple,
    /// or `&[(); 0]` for none.
    pub fn new(query: &str, params: &impl Serialize) -> Result<Self> {
        let params = serde_json::to_value(params)
            .map_err(|error| format!("unserializable Neon parameters: {error}"))?;
        if !params.is_array() {
            return Err("Neon parameters must serialize as a JSON array".into());
        }
        Ok(Self {
            query: query.into(),
            params,
        })
    }
}
