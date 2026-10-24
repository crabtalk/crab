//! The host every berm daemon runs: the services it holds, the state they
//! commit, and the calls into them. A daemon builds a [`Host`] over its own
//! [`Env`], authenticates its callers, nests [`router`] under the path it names
//! services by and serves its own routes beside it; everything a program
//! observes is the host's.

use axum::{
    Json,
    http::{HeaderValue, StatusCode, header},
    response::IntoResponse,
};
use berm_api::Failed;
use std::fmt;

pub use api::{
    outside::{Call, Called, Layers, Outside},
    serve::{Named, router},
};
pub use config::Config;
pub use env::{Clock, Edges, Env, Images, Observe, Peers, Secrets, System};
pub use host::Host;
pub use images::Stored;
pub use state::store::Store;

mod api;
mod config;
mod env;
mod host;
mod images;
mod record;
mod state;
#[cfg(feature = "test")]
pub mod test;
mod wake;

/// A refusal the caller can read, as `{"error": message}`.
pub fn deny(status: StatusCode, message: impl Into<String>) -> Denied {
    Denied(status, message.into())
}

#[derive(Debug)]
pub struct Denied(pub StatusCode, pub String);

impl IntoResponse for Denied {
    fn into_response(self) -> axum::response::Response {
        let mut response = (self.0, Json(Failed { error: self.1 })).into_response();
        // Only a tool answers `405`, and every tool takes `POST`.
        if self.0 == StatusCode::METHOD_NOT_ALLOWED {
            response
                .headers_mut()
                .insert(header::ALLOW, HeaderValue::from_static("POST"));
        }
        response
    }
}

impl fmt::Display for Denied {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.1)
    }
}

impl std::error::Error for Denied {}

impl From<anyhow::Error> for Denied {
    fn from(e: anyhow::Error) -> Self {
        tracing::error!("host failed: {e:#}");
        deny(StatusCode::INTERNAL_SERVER_ERROR, "internal error")
    }
}
