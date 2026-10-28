//! Organization and membership wire types.

use crate::{
    account::{auth::Client, billing::SubscriptionStatus},
    utils::text_enum,
};
use serde::{Deserialize, Serialize};

/// What a member may do in an org. An owner administers it — members, teams,
/// keys, billing, and publishing; everything else is `member`.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Owner,
    #[default]
    Member,
}
text_enum!(Role { Owner => "owner", Member => "member" });

#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Org {
    pub id: String,
    /// Whether this account is a person or a shared org. Mirrors GitHub's
    /// `"type": "User" | "Organization"`, and like theirs it is what a client
    /// branches on to pick an avatar over an org glyph.
    #[serde(rename = "type")]
    pub kind: Kind,
    /// URL handle, unique across every account — people and shared orgs share
    /// one namespace, so `/<handle>` is never ambiguous.
    /// `None` until one is picked.
    pub handle: Option<String>,
    pub name: String,
    /// Image URL for the switcher and any list that names this account. Set
    /// from the OAuth provider for a person and by `PATCH /orgs/{org}` for a
    /// shared org, so it is `None` until someone picks one.
    pub avatar: Option<String>,
    /// What the caller may do here. Every org screen branches on it — the
    /// handle field, the invite form, delete — so it rides the
    /// org rather than being read back out of the roster, which the screens
    /// that need it otherwise have no reason to load.
    pub role: Role,
    pub created_at: i64,
    pub updated_at: i64,
}

/// One account, as much of it as the caller may see. `GET /orgs/{org}`.
///
/// Identity is always there. `private` is present exactly when the caller is a
/// member, so its absence is the answer to "am I in this org", and a client
/// needs no second call to find out.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrgView {
    #[serde(flatten)]
    pub account: PublicAccount,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub private: Option<OrgPrivate>,
}

/// What membership buys: the org's plan and its timestamps. Absent for everyone else.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrgPrivate {
    /// What the caller may do here. Present exactly when they are a member,
    /// since that is when `private` is present at all.
    pub role: Role,
    pub updated_at: i64,
    /// The account's latest subscription regardless of status, so a client
    /// can branch on `status`: `past_due` → access is already paused (no
    /// grace); show a "payment failed, update card to restore" banner.
    /// `canceled`/`unpaid` → the sub ended, prompt resubscribe. `plan` is the
    /// real entitlement (the granted rung once non-`active`). `null` means never
    /// subscribed.
    pub subscription: Option<Subscription>,
    /// Active plan slug, as `GET /meta/plans` names it. Every account holds one:
    /// a subscription buys its own, and the rung an account's kind is granted
    /// answers for the rest. Being a member of a paying org does not upgrade
    /// your own.
    pub plan: String,
}

#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subscription {
    /// Stripe subscription status the client branches on: `active`/`trialing`
    /// entitle the paid plan, and `trialing` is held only by a trial started
    /// before trials were removed; `past_due` means a renewal failed and access is
    /// paused (no grace) — prompt the user to update their card; `canceled`/
    /// `unpaid` mean the subscription ended — prompt resubscribe.
    pub status: SubscriptionStatus,
    /// The plan slug this subscription buys; `None` for a plan the deployment
    /// no longer sells.
    pub plan: Option<String>,
    pub current_period_end: Option<i64>,
    /// When a cancelled subscription stops, or `null` while it renews.
    ///
    /// Cancelling leaves `status` on `active` for the rest of the period already
    /// paid for, so this is what says whether the account is renewing or running
    /// out. Access is unchanged until it passes.
    pub cancel_at: Option<i64>,
    /// The plan slug it changes to at `current_period_end`, or `null` when no
    /// change is scheduled.
    #[serde(default)]
    pub next_plan: Option<String>,
}

/// Which kind of account: a person or a shared org.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    User,
    Org,
}

text_enum!(Kind { User => "user", Org => "org" });

/// An account as anyone may see it — a person or an org. Identity only: nothing about what funds it, who is in it, or what it
/// holds. This is what `GET /orgs/{org}` answers without a session, so knowing a
/// handle discloses its profile, the way `github.com/rust-lang` does.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicAccount {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: Kind,
    pub handle: Option<String>,
    pub name: String,
    pub avatar: Option<String>,
    /// A line about the account, set by its owners.
    pub bio: Option<String>,
    /// `https://` URLs its owners list, in their order.
    pub links: Vec<String>,
    pub created_at: i64,
}

/// A membership, with the person holding it. The profile fields are the only
/// way a client learns who `member_id` is: `GET /orgs/{member_id}` is member-gated, and nobody is a
/// member of someone else's account.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Member {
    pub org_id: String,
    pub user_id: String,
    pub role: Role,
    /// When they joined this org, not when their account was created.
    pub created_at: i64,
    pub name: String,
    pub handle: Option<String>,
    pub avatar: Option<String>,
    /// `None` only for a member that is itself an org; a person always has one.
    pub email: Option<String>,
}

#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateOrgRequest {
    pub name: String,
    /// URL handle, claimed with the org: 1–39 chars of `[a-z0-9-]`, not
    /// starting or ending with `-`. One namespace covers people and orgs.
    pub handle: String,
    /// Optional plan slug. When set to a paid plan, the response includes a
    /// Stripe Checkout URL the frontend should redirect to. Omit and the org
    /// holds the granted rung every org starts on.
    #[serde(default)]
    pub tier: Option<String>,
    /// Surface that will receive the post-checkout redirect. Required when
    /// `tier` is a paid plan; ignored otherwise.
    #[serde(default)]
    pub client: Option<Client>,
}

#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateOrgResponse {
    #[serde(flatten)]
    pub org: Org,
    /// Stripe Checkout URL when a paid tier was requested at creation.
    /// Frontend should navigate the user here to complete payment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checkout_url: Option<String>,
}

/// Partial update of an org. Every field is optional; an absent one is left
/// alone.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateOrgRequest {
    /// URL handle: 1–39 chars, `[a-z0-9-]`, can't start or end with `-`.
    /// A handle is never unset once picked, so `null` reads the same as
    /// absent — no change — and this field needs no nesting.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub handle: Option<String>,
    /// Display name. 1–100 characters after trimming whitespace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
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

/// A group of an org's members that gates access. Membership overlaps: a
/// person can hold several teams.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Team {
    pub org_id: String,
    /// URL segment, unique within the org: 1–39 chars of `[a-z0-9-]`.
    pub slug: String,
    pub name: String,
    /// How many of the org's members hold this team.
    pub member_count: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamMember {
    pub org_id: String,
    pub team: String,
    pub user_id: String,
    /// When they joined this team, not when their account was created.
    pub created_at: i64,
    pub name: String,
    pub handle: Option<String>,
    pub avatar: Option<String>,
    /// `None` only for a member that is itself an org; a person always has one.
    pub email: Option<String>,
}

#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTeamRequest {
    /// 1–39 chars of `[a-z0-9-]`, not starting or ending with `-`.
    pub slug: String,
    pub name: String,
}

/// Partial update of a team. Absent fields are left alone.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateTeamRequest {
    /// A new slug: the segment in the team's URL. Its memberships follow it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slug: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// A pending invitation to join an org. Becomes a [`Member`] when the
/// invited address accepts.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Invite {
    pub org_id: String,
    /// Org name, so a client listing the caller's own invitations can
    /// say what they're being invited to without a second call.
    pub org_name: String,
    pub email: String,
    pub role: Role,
    /// Unguessable acceptance credential. Cloud sends no mail, so the owner
    /// delivers this themselves. It does not stand alone — accepting also
    /// requires the caller's verified email to match `email`.
    pub token: String,
    pub created_at: i64,
    pub expires_at: i64,
}

#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InviteRequest {
    /// Address to invite. Matched against the verified email of whoever
    /// accepts, and against a new account's on first login.
    pub email: String,
    #[serde(default)]
    pub role: Role,
}

#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddMemberRequest {
    pub email: String,
    #[serde(default)]
    pub role: Role,
}

#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateMemberRequest {
    pub role: Role,
}

/// Where an uploaded picture landed. The same value `avatar` now holds, so a
/// client can render it without re-reading the account.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvatarResponse {
    pub avatar: String,
}
