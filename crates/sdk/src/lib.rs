//! Cloud's client-facing API wire types.
//!
//! One home for the request/response shapes the cloud server produces, the
//! OpenAPI spec derives from, and a consumer reads — so a field added
//! server-side can't silently drift out of a hand-copied client type.
//!
//! `serde` always, `utoipa::ToSchema` behind the `utoipa` feature, and the
//! HTTP [`client`] behind `client`.

pub use berm_api;

pub mod account;
#[cfg(feature = "client")]
pub mod client;
pub mod error;
pub mod mcp;
pub mod services;
pub mod system;
