//! A guest with no declared HTTP destinations.

#![cfg_attr(target_arch = "wasm32", no_std)]

extern crate alloc;

use berm_lang::{Result, tool};

/// Requests `https://example.com/`, answering its status.
#[tool]
pub fn fetch() -> Result<u16> {
    Ok(berm_lang::http::fetch("GET", "https://example.com/", &[], b"")?.status)
}

/// Vouches for the user `someone`.
#[tool]
pub fn vouch() -> Result<()> {
    Ok(berm_lang::vouch("someone")?)
}

/// Read the outside IP.
#[tool]
pub fn client_ip() -> Result<()> {
    berm_lang::client_ip()?;
    Ok(())
}
