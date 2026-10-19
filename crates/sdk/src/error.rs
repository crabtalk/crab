//! Error envelopes.
//!
//! Wire types, so they live here rather than beside the server's error carrier:
//! every service crate needs to *produce* one, and only cloud has an
//! `AppError`. Keeping the shapes in the shared crate is what lets a refusal
//! from a service crate look identical to one from cloud.

use serde::{Deserialize, Serialize};

/// Plain error envelope — `{"error": "message"}`.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
}

/// Structured envelope for typed conditions the client must branch on (e.g.
/// `orgs_closed`). Distinguished from
/// [`ErrorResponse`] by the nested object: clients switch on `error.code`, not
/// on the HTTP status, when surfacing UX.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiError {
    pub error: ApiErrorBody,
}

#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiErrorBody {
    /// Stable identifier the client matches on. Snake-case, never renamed.
    pub code: String,
    /// Human-readable explanation. Safe to display; never leaks internals.
    pub message: String,
    /// Unix timestamp (seconds, UTC) when the condition self-resolves. Omitted
    /// when the condition is not time-bounded — funding arrives on purchase,
    /// not on a clock.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reset_at: Option<i64>,
}
