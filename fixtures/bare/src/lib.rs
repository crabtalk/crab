//! The reference guest declaring nothing beyond the ABI. Each tool makes one
//! syscall a manifest has to declare, which the host refuses it.
//!
//! `sign` signs an ID token, and `issuer` names this program's issuer.

#![cfg_attr(target_arch = "wasm32", no_std, no_main)]

extern crate alloc;

#[berm_lang::program]
mod tools {
    use alloc::string::String;
    use berm_lang::{Result, serde_json::json};

    /// Signs a token for `sub` `s`, `aud` `a`, expiring at `exp` 1.
    #[public]
    pub fn sign() -> Result<String> {
        Ok(berm_lang::token::sign(
            &json!({"sub": "s", "aud": "a", "exp": 1}),
        )?)
    }

    /// Answers this program's issuer.
    #[public]
    pub fn issuer() -> Result<String> {
        Ok(berm_lang::token::issuer()?)
    }
}
