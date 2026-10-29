//! App wire types: what lets a website call services from its users' browsers.

use serde::{Deserialize, Serialize};

/// The origin that admits a browser on any origin, for an anonymous app with
/// no issuers.
pub const ANY_ORIGIN: &str = "*";

/// An app: the origins a browser may call from, the identity providers whose
/// tokens name its users, and the services it may call.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct App {
    pub id: String,
    pub owner_id: String,
    pub name: String,
    /// `scheme://host[:port]`, as a browser sends `Origin`, or [`ANY_ORIGIN`].
    pub origins: Vec<String>,
    pub issuers: Vec<Issuer>,
    /// Services it may call, as `{owner}/{name}`, the owner by its handle, or its
    /// id when it has none.
    pub services: Vec<String>,
    /// Calls with no token reach the services' `access = "anonymous"` tools.
    pub anonymous: bool,
    pub created_at: i64,
}

impl App {
    /// Whether a browser on `origin` may call through this app.
    pub fn admits(&self, origin: &str) -> bool {
        self.origins
            .iter()
            .any(|allowed| allowed == origin || allowed == ANY_ORIGIN)
    }
}

/// An identity provider whose ID tokens an app's calls carry.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Issuer {
    /// The token's `iss`. Its signing keys are read from its OpenID discovery
    /// document's `jwks_uri`.
    pub issuer: String,
    /// The token's `aud`.
    pub audience: String,
}

/// What an app is created or replaced with.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppRequest {
    pub name: String,
    pub origins: Vec<String>,
    #[serde(default)]
    pub issuers: Vec<Issuer>,
    /// Services it may call, as `{owner}/{name}`, the owner by handle or id.
    pub services: Vec<String>,
    #[serde(default)]
    pub anonymous: bool,
}
