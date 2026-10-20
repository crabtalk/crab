//! What Neon's endpoint answers for one statement, with
//! `Neon-Raw-Text-Output` and `Neon-Array-Mode` on: each row an array of its
//! columns in Postgres' text output, `null` for `NULL`.

use alloc::{format, string::String, vec::Vec};
use berm_lang::{
    Result,
    serde::{Deserialize, de::DeserializeOwned},
    serde_json::{self, Number, Value},
};

const BOOL: u32 = 16;
const INT8: u32 = 20;
const INT2: u32 = 21;
const INT4: u32 = 23;
const OID: u32 = 26;
const JSON: u32 = 114;
const FLOAT4: u32 = 700;
const FLOAT8: u32 = 701;
const JSONB: u32 = 3802;

/// One statement's result.
#[derive(Deserialize)]
#[serde(crate = "berm_lang::serde")]
pub struct Answer {
    pub fields: Vec<Field>,
    /// The command tag's first word, e.g. `SELECT` or `INSERT`.
    pub command: String,
    /// The rows the command reports, `None` for a command that reports none.
    #[serde(rename = "rowCount")]
    pub row_count: Option<u64>,
    #[serde(rename = "rows")]
    text: Vec<Vec<Option<String>>>,
}

/// A result column.
#[derive(Deserialize)]
#[serde(crate = "berm_lang::serde")]
pub struct Field {
    pub name: String,
    /// The column type's Postgres OID.
    #[serde(rename = "dataTypeID")]
    pub data_type: u32,
}

impl Answer {
    /// The rows, each a tuple of its columns in order.
    ///
    /// A column is read as JSON by its type: `bool` a boolean; `int2`,
    /// `int4`, `int8` and `oid` an integer; `float4` and `float8` a number,
    /// or a string when not finite; `json` and `jsonb` their value; any
    /// other type its text.
    pub fn rows<T: DeserializeOwned>(&self) -> Result<Vec<T>> {
        let rows = self
            .text
            .iter()
            .map(|row| {
                row.iter()
                    .zip(&self.fields)
                    .map(|(text, field)| field.value(text.as_deref()))
                    .collect::<Result<Vec<_>>>()
                    .map(Value::Array)
            })
            .collect::<Result<Vec<_>>>()?;
        serde_json::from_value(Value::Array(rows))
            .map_err(|error| format!("Neon rows do not decode: {error}").into())
    }
}

impl Field {
    /// `text`, this column's value in Postgres' text output, as JSON.
    fn value(&self, text: Option<&str>) -> Result<Value> {
        let Some(text) = text else {
            return Ok(Value::Null);
        };
        let unreadable = || format!("Neon answered an unreadable `{}`: {text}", self.name);
        Ok(match self.data_type {
            BOOL => Value::Bool(matches!(
                text,
                "TRUE" | "t" | "true" | "y" | "yes" | "on" | "1"
            )),
            INT2 | INT4 | INT8 | OID => Value::from(text.parse::<i64>().map_err(|_| unreadable())?),
            FLOAT4 | FLOAT8 => {
                let float = text.parse::<f64>().map_err(|_| unreadable())?;
                Number::from_f64(float).map_or_else(|| Value::String(text.into()), Value::Number)
            }
            JSON | JSONB => serde_json::from_str(text).map_err(|_| unreadable())?,
            _ => Value::String(text.into()),
        })
    }
}
