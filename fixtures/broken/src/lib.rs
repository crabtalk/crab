//! The reference guest whose migration fails.

#![cfg_attr(target_arch = "wasm32", no_std)]

extern crate alloc;

use berm_lang::{Result, tool};

/// Answers nothing.
#[tool(public)]
pub fn ping() -> Result<()> {
    Ok(())
}
