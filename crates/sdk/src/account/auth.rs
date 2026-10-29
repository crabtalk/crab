//! `/auth` — signing in, the sessions it mints, and the provider accounts
//! it signs in with.

use serde::{Deserialize, Serialize};

/// Which front-end surface started the sign-in: where the callback redirects,
/// and what the session it mints reports as its `client`.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Client {
    Web,
    Terminal,
}

berm_api::text_enum!(Client);

#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub user_id: String,
    pub client: Client,
    pub ip: String,
    pub agent: String,
    pub created_at: i64,
    /// Unix seconds when the session lapses *if left idle*. Every
    /// authenticated request pushes it forward to `now + 7d`.
    pub expires_at: i64,
    /// Unix seconds the session was explicitly revoked (logout / "sign out
    /// this device"), or null if still live. A revoked session's token is
    /// rejected immediately, regardless of `expires_at`.
    pub revoked_at: Option<i64>,
    /// Unix seconds of the most recent authenticated request, or null if the
    /// session has not been used since creation. Drives the sliding
    /// `expires_at` and the "last active" display.
    pub last_used_at: Option<i64>,
}

/// One way the caller signs in — a provider account attached to this one.
///
/// Keyed on the provider's own subject, which is stable across a rename there;
/// `email` is what that provider reported when the connection was made, and is
/// carried for display.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Connection {
    /// `google`, `github`, or `email` for a code emailed to the address.
    pub provider: String,
    /// Null when the provider shared no address.
    pub email: Option<String>,
    pub created_at: i64,
}

/// Redirect URL handed back to a logged-in client that wants to link a
/// provider account to its current user.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkResponse {
    /// Provider consent URL the FE should navigate the browser to.
    pub redirect_url: String,
}
