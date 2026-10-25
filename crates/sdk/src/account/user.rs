//! Wire types for the caller: profile, sessions, login connections, and what
//! they belong to.

use crate::account::org::{Org, Subscription};
use serde::{Deserialize, Serialize};

#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    /// GitHub-style URL handle, unique across every account. `None` until one
    /// is picked on first login.
    pub handle: Option<String>,
    pub name: String,
    pub avatar: Option<String>,
    /// A line about the person, shown on their profile.
    pub bio: Option<String>,
    /// `https://` URLs shown on their profile, in their order.
    pub links: Vec<String>,
    pub email: String,
    pub created_at: i64,
    pub updated_at: i64,
}

/// The caller's own account, in full, and the orgs they belong to.
///
/// A person's account is not an org, so this is the only place it is answered:
/// `/orgs/{org}` resolves orgs and `404`s a person. Everything an org gets from
/// `GET /orgs/{org}` is here for the account the token names — what funds it —
/// so a client builds its account switcher from this one response and never
/// reconciles two shapes.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthMe {
    #[serde(flatten)]
    pub user: User,
    /// Active plan slug, as `GET /meta/plans` names it: the subscription's when
    /// one is live, the granted rung otherwise.
    pub plan: String,
    /// The account's latest subscription regardless of status, read the same
    /// way as an org's: `past_due` means access is already paused, `canceled`
    /// and `unpaid` mean it ended. `plan` is the entitlement.
    pub subscription: Option<Subscription>,
    pub orgs: Vec<Org>,
}

/// Partial update of the authenticated user. Every field is optional; an
/// absent one is left alone.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateUserRequest {
    /// Display name. 1–100 characters after trimming whitespace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// URL handle: 1–39 chars, `[a-z0-9-]`, can't start or end with `-`.
    /// One namespace covers people and orgs alike, so this can collide with an
    /// org's. A handle is never unset once picked, so `null` reads the same as
    /// absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub handle: Option<String>,
    /// `https://` URL of an image to show wherever this account is named.
    /// Absent and `null` both leave it alone, as `handle` does.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar: Option<String>,
    /// At most 160 characters after trimming whitespace; empty clears it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bio: Option<String>,
    /// Replaces every link: at most 4 `https://` URLs under 512 characters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub links: Option<Vec<String>>,
}
