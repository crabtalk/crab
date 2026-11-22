//! __NAME__ — a berm service.

#![cfg_attr(target_arch = "wasm32", no_std)]

extern crate alloc;

use alloc::string::String;
use berm_lang::{Result, Schema, tool};

/// Echo a query
///
/// Answers its argument back.
#[tool]
pub fn echo(
    /// The text to echo back.
    query: String,
) -> Result<Echo> {
    Ok(Echo { query })
}

/// What `echo` answers.
#[derive(Schema)]
pub struct Echo {
    pub query: String,
}

const MESSAGE: &str = "message";

/// Store a message
///
/// Survives calls and restarts, shared by all callers.
#[tool]
pub fn remember(message: String) -> Result<()> {
    berm_lang::set(MESSAGE, &message)?;
    Ok(())
}

/// Read a saved message
///
/// Returns null if none has been saved.
#[tool(query)]
pub fn recall() -> Result<Option<String>> {
    Ok(berm_lang::get(MESSAGE)?)
}
