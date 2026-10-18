//! The service's database, from `migrations/`.

use alloc::vec::Vec;
use berm_lang::{
    CallError, json,
    serde::{Deserialize, Serialize, de::DeserializeOwned},
};

/// What `berm.sql` answers, each row as `T`, a tuple of its columns in order.
#[derive(Deserialize)]
#[serde(crate = "berm_lang::serde")]
struct Answer<T> {
    rows: Vec<T>,
}

/// Run `statement` with `params`, a tuple bound to its `?`s in order, and
/// answer its rows.
pub fn sql<T: DeserializeOwned>(
    statement: &str,
    params: &impl Serialize,
) -> Result<Vec<T>, CallError> {
    let answer: Answer<T> = json::from(&berm_lang::sql(statement, &json::to(params)?)?)?;
    Ok(answer.rows)
}
