//! The host every berm daemon runs: the services it holds, the state they
//! commit, and the calls into them. A daemon builds a [`Host`] over its own
//! [`Env`], authenticates its callers, nests [`router`] under the path it names
//! services by and serves its own routes beside it; everything a service
//! observes is the host's.

use crate::workflow::Refused as WorkflowRefused;
use axum::{
    Json,
    body::Body,
    http::{HeaderValue, Response as HttpResponse, StatusCode, header},
    response::{IntoResponse, Response},
};
use berm_api::{
    Failed,
    workflow::{
        config::Invalid,
        run::{Failure, Run},
        spend::Context as SpendContext,
    },
};
use serde_json::Value;
use std::fmt;

pub use api::{
    outside::{Call, Called, Layers, Outside},
    serve::{Named, Scoped, router, scope_router},
};
pub use config::Config;
pub use env::{Alone, Clock, Edges, Env, Images, Observe, Peers, Secrets, System};
pub use host::{Host, Installed};
pub use images::Stored;
pub use state::store::Store;

mod api;
mod config;
mod env;
mod events;
pub mod execution;
pub mod fixture;
mod host;
mod images;
mod record;
mod state;
#[cfg(feature = "test")]
pub mod test;
mod wake;
pub mod workflow;

/// A refusal the caller can read, as `{"error": message}`.
pub fn deny(status: StatusCode, message: impl Into<String>) -> Denied {
    Denied {
        status,
        message: message.into(),
        explain: None,
        failure: None,
    }
}

#[derive(Debug)]
pub struct Denied {
    pub status: StatusCode,
    pub failure: Option<Failure>,
    pub message: String,
    /// The error code explaining it, as `E0013`.
    pub explain: Option<&'static str>,
}

impl Denied {
    pub fn manifest(error: anyhow::Error) -> Self {
        let explain = Invalid::explanation(&error);
        Self {
            explain,
            ..deny(StatusCode::BAD_REQUEST, format!("{error:#}"))
        }
    }

    pub(crate) const SERVICE_CALLERS: &'static str = "E0049";

    pub(crate) const PAUSED: &'static str = "E0018";

    pub(crate) const MEMORY: &str = "E0019";

    pub(crate) fn memory() -> Self {
        deny(
            StatusCode::TOO_MANY_REQUESTS,
            "the account holds as much memory as it may; retry",
        )
        .explained(Self::MEMORY)
    }

    /// It, explained by `code`.
    pub fn explained(self, code: &'static str) -> Self {
        Self {
            explain: Some(code),
            ..self
        }
    }
}

impl IntoResponse for Denied {
    fn into_response(self) -> Response {
        if let Some(failure) = self.failure {
            return match HttpResponse::<Vec<u8>>::try_from(failure) {
                Ok(response) => response.map(Body::from),
                Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
            };
        }
        let failed = Failed {
            error: self.message,
            explain: self.explain.map(str::to_owned),
        };
        let mut response = (self.status, Json(failed)).into_response();
        // Only a tool answers `405`, and every tool takes `POST`.
        if self.status == StatusCode::METHOD_NOT_ALLOWED {
            response
                .headers_mut()
                .insert(header::ALLOW, HeaderValue::from_static("POST"));
        }
        response
    }
}

impl fmt::Display for Denied {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Denied {}

impl From<anyhow::Error> for Denied {
    fn from(e: anyhow::Error) -> Self {
        if let Some(failure) = e.downcast_ref::<Failure>() {
            return failure.clone().into();
        }
        if let Some(refused) = e.downcast_ref::<WorkflowRefused>() {
            return refused.clone().into();
        }
        if e.is::<berm::MemoryLimit>() {
            return deny(StatusCode::TOO_MANY_REQUESTS, e.to_string()).explained(Self::MEMORY);
        }
        match e.downcast::<Self>() {
            Ok(denied) => denied,
            Err(e) => {
                tracing::error!("host failed: {e:#}");
                deny(StatusCode::INTERNAL_SERVER_ERROR, "internal error")
            }
        }
    }
}

impl From<&Denied> for Failure {
    fn from(denied: &Denied) -> Self {
        if let Some(failure) = &denied.failure {
            return failure.clone();
        }
        let mut body = serde_json::json!({"error":denied.message});
        if let Some(explain) = denied.explain {
            body["explain"] = explain.into();
        }
        Self {
            status: denied.status.as_u16(),
            headers: Default::default(),
            body,
        }
    }
}

impl From<Failure> for Denied {
    fn from(failure: Failure) -> Self {
        let mut denied = deny(
            StatusCode::from_u16(failure.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            failure
                .body
                .get("error")
                .and_then(Value::as_str)
                .unwrap_or("request refused"),
        );
        denied.failure = Some(failure);
        denied
    }
}

impl From<WorkflowRefused> for Denied {
    fn from(error: WorkflowRefused) -> Self {
        let message = error.to_string();
        match error {
            WorkflowRefused::Capacity => {
                deny(StatusCode::FORBIDDEN, message).explained(Run::CAPACITY)
            }
            WorkflowRefused::Missing => deny(StatusCode::NOT_FOUND, message),
            WorkflowRefused::Busy => deny(StatusCode::CONFLICT, message).explained(Run::BUSY),
            WorkflowRefused::Expired => deny(StatusCode::GONE, message).explained(Run::EXPIRED),
            WorkflowRefused::Removed => deny(StatusCode::GONE, message).explained(Run::REMOVED),
            WorkflowRefused::Ineligible => {
                deny(StatusCode::CONFLICT, message).explained(Run::FINISHED)
            }
            WorkflowRefused::Deadline => {
                deny(StatusCode::CONFLICT, message).explained(SpendContext::EXPIRED)
            }
            WorkflowRefused::Cancelled => {
                deny(StatusCode::CONFLICT, message).explained(SpendContext::CANCELLED)
            }
            WorkflowRefused::Unsafe(_) => {
                deny(StatusCode::CONFLICT, message).explained(Run::UNSAFE)
            }
            WorkflowRefused::Invalid(_) => deny(StatusCode::UNPROCESSABLE_ENTITY, message),
        }
    }
}
