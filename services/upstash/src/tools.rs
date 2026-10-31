use crate::{CommandResult, Redis};
use alloc::{string::String, vec::Vec};
use berm_lang::{Result, tool};
fn database() -> Result<Redis> {
    Redis::new(&berm_lang::var("UPSTASH_REDIS_REST_URL")?)
}

/// Read a Redis string. A missing key returns null.
#[tool]
pub fn get(
    /// Redis key.
    key: String,
) -> Result<Option<String>> {
    database()?.get(&key)
}

/// Write a Redis string, optionally with an expiry.
#[tool]
pub fn set(
    /// Redis key.
    key: String,
    /// String to store.
    value: String,
    /// Expiry in seconds, greater than zero; omit for no expiry.
    expiry_seconds: Option<u64>,
) -> Result<()> {
    database()?.set(&key, &value, expiry_seconds)
}

/// Delete Redis keys and return how many existed.
#[tool]
pub fn delete(
    /// Keys to delete.
    keys: Vec<String>,
) -> Result<u64> {
    database()?.delete(&keys)
}

/// Change a key's expiry; zero deletes it immediately.
#[tool]
pub fn expire(
    /// Redis key.
    key: String,
    /// Seconds until expiry.
    seconds: u64,
) -> Result<bool> {
    database()?.expire(&key, seconds)
}

/// Read remaining seconds. -1 means persistent; -2 means missing.
#[tool]
pub fn ttl(
    /// Redis key.
    key: String,
) -> Result<i64> {
    database()?.ttl(&key)
}

/// Increment a Redis integer by a signed amount. Do not blindly retry failures.
#[tool]
pub fn increment(
    /// Redis key.
    key: String,
    /// Positive or negative amount to add.
    amount: i64,
) -> Result<i64> {
    database()?.increment(&key, amount)
}

/// Run Redis commands in order, with possible interleaving. Inspect each returned error.
#[tool]
pub fn pipeline(
    /// Each command is an array of strings, starting with its name.
    commands: Vec<Vec<String>>,
) -> Result<Vec<CommandResult>> {
    database()?.pipeline(&commands)
}

/// Run Redis commands without interleaving. Runtime errors do not roll back other commands.
#[tool]
pub fn transaction(
    /// Each command is an array of strings, starting with its name.
    commands: Vec<Vec<String>>,
) -> Result<Vec<CommandResult>> {
    database()?.transaction(&commands)
}
