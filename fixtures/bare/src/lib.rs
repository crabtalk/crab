//! The reference guest declaring nothing beyond the ABI. Each tool makes one
//! syscall a manifest has to declare, which the host refuses it.
//!
//! `fetch` requests a host it does not declare.

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
}
