//! TinyFish search, extraction and asynchronous browser-agent tools.
#![no_std]

extern crate alloc;

pub mod api;
mod bindings;
pub mod types;

#[cfg(feature = "tools")]
pub mod tools;
