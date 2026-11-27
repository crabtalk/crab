//! hello — a greeting service.

#![cfg_attr(target_arch = "wasm32", no_std)]

extern crate alloc;

use alloc::{format, string::String};
use berm_lang::{Result, Schema, tool};

/// Say hello
///
/// Greets `name`, or the world when it is omitted.
#[tool(query, access = "anonymous", budget(cpu = 1_000, memory = 2))]
pub fn hello(
    /// Who to greet.
    name: Option<String>,
) -> Result<Greeting> {
    let name = name.as_deref().unwrap_or("world");
    Ok(Greeting {
        message: format!("Hello, {name}!"),
    })
}

/// What `hello` answers.
#[derive(Schema)]
pub struct Greeting {
    pub message: String,
}
