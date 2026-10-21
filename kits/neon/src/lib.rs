//! Neon's SQL over HTTP, through `berm.http.fetch`, with the password in the
//! secret `NEON_PASSWORD`.
//!
//! The kit declares no host. A program using it declares its project's
//! endpoint under `berm.http.fetch`: `api.` and the connection string's host
//! after its first label.

#![no_std]

extern crate alloc;

berm_lang::kit!(secrets = ["NEON_PASSWORD"]);

mod answer;
mod neon;
mod statement;

pub use answer::{Answer, Field};
pub use neon::Neon;
pub use statement::Statement;
