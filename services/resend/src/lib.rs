//! Resend email sending and delivery status for berm services.
#![no_std]

extern crate alloc;

mod api;
mod types;
pub use api::*;
pub use types::*;

#[cfg(feature = "tools")]
pub mod tools;
