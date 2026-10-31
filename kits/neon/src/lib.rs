//! Neon's SQL over HTTP, through `berm.http.fetch`, with the password in the
//! secret `NEON_PASSWORD`.
//!
//! The kit declares no host. A service using it declares its project's
//! endpoint under `berm.http.fetch`: `api.` and the connection string's host
//! after its first label.

#![no_std]

extern crate alloc;

mod answer;
mod neon;
mod statement;

pub use answer::{Answer, Field};
pub use neon::Neon;
pub use statement::Statement;
