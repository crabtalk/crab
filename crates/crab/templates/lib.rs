//! __NAME__ — a berm program.

// `no_std` is the guest's shape. Off a guest target this is an ordinary
// library, so `cargo test` runs the tools below natively.
#![cfg_attr(target_arch = "wasm32", no_std)]

extern crate alloc;

use alloc::string::String;
use berm_lang::{Result, Schema, tool};

/// Answers its argument back.
#[tool(public)]
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
