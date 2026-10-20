//! Neon's SQL over HTTP, through `berm.http.fetch`, with the password in the
//! secret `NEON_PASSWORD`.
//!
//! The endpoint's host differs per Neon project, so a program using this kit
//! declares it under `berm.http.fetch` itself: `api.` and the connection
//! string's host after its first label.

#![no_std]

extern crate alloc;

berm_lang::kit!(secrets = ["NEON_PASSWORD"]);

mod answer;
mod neon;
mod statement;

pub use answer::{Answer, Field};
pub use neon::Neon;
pub use statement::Statement;
