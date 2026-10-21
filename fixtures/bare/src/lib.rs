//! The reference guest declaring nothing beyond the ABI. Each tool makes one
//! syscall a manifest has to declare, which the host refuses it.
//!
//! `fetch` requests a host it does not declare, and `vouch` vouches for a user.

#![cfg_attr(target_arch = "wasm32", no_std, no_main)]

extern crate alloc;

#[berm_lang::program]
mod tools {
    use berm_lang::Result;

    /// Requests `https://example.com/`, answering its status.
    #[public]
    pub fn fetch() -> Result<u16> {
        Ok(berm_lang::http::fetch("GET", "https://example.com/", &[], b"")?.status)
    }

    /// Vouches for the user `someone`.
    #[public]
    pub fn vouch() -> Result<()> {
        Ok(berm_lang::vouch("someone")?)
    }
}
