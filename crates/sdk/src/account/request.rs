//! `/requests` — what an account asks a person to do, handed to them as a
//! short code: pay for a purchase, set secrets of a service, sign a device in
//! as them, approve a deploy, or verify on GitHub.

use crate::{account::org::PublicAccount, services::service::DeployRequest};
use berm_api::{Credential, Deployed, Manifest};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// What a request asks of the person.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RequestKind {
    /// Pay for a purchase.
    Payment,
    /// Set secrets of a service.
    Secret,
    /// Sign a device in as the person: RFC 8628's device grant.
    Login,
    /// Approve a deploy of the account's services, which runs as the person
    /// approving it.
    Deploy,
    /// Verify the person on GitHub, which lets their account deploy on Hobby.
    Verify,
}

berm_api::text_enum!(RequestKind);

/// What a payment request buys.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Purchase {
    /// A paid plan's subscription.
    Plan,
    /// Credit, in an amount chosen at checkout.
    Credit,
}

berm_api::text_enum!(Purchase);

/// Where a request stands.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RequestStatus {
    /// Waiting on the person.
    Pending,
    /// Paid, the secrets set, the sign-in approved, the deploy approved and
    /// run, whether or not it went live, or the person verified.
    Completed,
    /// The person declined it.
    Declined,
    /// Left pending past `expires_at`.
    Expired,
}

berm_api::text_enum!(RequestStatus);

/// What to ask a person, at `POST /requests`, for the account `account` names
/// by handle or id.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum CreateRequest {
    /// Pay for a purchase.
    Payment {
        account: String,
        purchase: Purchase,
        /// The plan a `plan` purchase subscribes to.
        #[serde(default)]
        plan: Option<String>,
        /// What it is for, in the asker's words, shown to the person.
        #[serde(default)]
        context: String,
    },
    /// Set secrets of one of the account's services: every credential its
    /// runtime declares with a secret unset, and each unset secret in no
    /// credential, or the one `credential` names, set or not.
    Secret {
        account: String,
        service: String,
        #[serde(default)]
        credential: Option<String>,
        /// The values its credentials' `copy` labels, by key; one fills
        /// every credential declaring its key.
        #[serde(default)]
        copy: BTreeMap<String, String>,
        /// What it is for, in the asker's words, shown to the person.
        #[serde(default)]
        context: String,
    },
    /// Deploy the account's services together, by name, once a person
    /// approves; as `POST /orgs/{org}/services/deploys` deploys them.
    Deploy {
        account: String,
        services: BTreeMap<String, DeployRequest>,
        /// What it is for, in the asker's words, shown to the person.
        #[serde(default)]
        context: String,
    },
    /// Verify the person `account` names on GitHub. It completes once they
    /// are verified, by its page or any other way.
    Verify {
        account: String,
        /// What it is for, in the asker's words, shown to the person.
        #[serde(default)]
        context: String,
    },
}

/// A request, as the person who opens its code sees it.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    /// What a person types at `/pay`, `/secrets`, `/device`, `/deploy` or
    /// `/verify`, as `WDJB-MJHT`.
    pub code: String,
    /// The page that opens it.
    pub url: String,
    pub kind: RequestKind,
    pub status: RequestStatus,
    /// The account it is for; none for a sign-in until a person approves it.
    pub account: Option<PublicAccount>,
    pub context: String,
    pub created_at: i64,
    pub expires_at: i64,
    /// What a payment request buys.
    pub purchase: Option<Purchase>,
    /// The plan a `plan` purchase subscribes to.
    pub plan: Option<String>,
    /// The plan's monthly price in microdollars; none for credit, whose
    /// amount is chosen at checkout.
    pub price_micros: Option<i64>,
    /// The service a secret request sets secrets of.
    pub service: Option<String>,
    /// What a secret request asks for, each set on its own. A secret in no
    /// credential is one of its own, by the secret's name, with no title.
    #[serde(default)]
    pub credentials: Vec<AskedCredential>,
    /// The registered client a sign-in is for, by its name; none for a
    /// device's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client: Option<String>,
    /// Where a client's sign-in sends the person back to once approved or
    /// declined, its code or error on it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub redirect: Option<String>,
    /// What a deploy request deploys, by service.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub deploys: BTreeMap<String, DeployRequest>,
    /// What each image a deploy request deploys says it is, by service, read
    /// from the image when it was asked.
    #[cfg_attr(feature = "utoipa", schema(value_type = Object))]
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub manifests: BTreeMap<String, Manifest>,
    /// Each service an approved deploy put live, by name.
    #[cfg_attr(feature = "utoipa", schema(value_type = Object))]
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub deployed: BTreeMap<String, Deployed>,
    /// Why an approved deploy did not go live.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure: Option<String>,
    /// The account is a person's on Hobby, not verified: a deploy request is
    /// refused when approved until they verify.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub unverified: bool,
}

/// A credential a secret request asks for, as its service declared it when
/// the request was made.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AskedCredential {
    pub id: String,
    #[serde(default)]
    pub title: String,
    /// How to get its secrets, in Markdown.
    #[serde(default)]
    pub guide: String,
    /// What the person enters at the provider, from the request's `copy`.
    #[serde(default)]
    pub copy: Vec<CopyValue>,
    /// The hosts its secrets go to: those its set secrets go to, else those
    /// its service suggests. The person confirms them on saving.
    #[serde(default)]
    pub hosts: Vec<String>,
    pub secrets: Vec<AskedSecret>,
    /// Set through this request. The request completes once every one is.
    #[serde(default)]
    pub saved: bool,
}

impl AskedCredential {
    /// `credential`, declared as `id`, with the values `copy` gives its copy
    /// keys; a key `copy` gives no value is left out.
    pub fn new(id: String, credential: Credential, copy: &BTreeMap<String, String>) -> Self {
        Self {
            id,
            title: credential.title,
            guide: credential.guide,
            copy: credential
                .copy
                .into_iter()
                .filter_map(|(key, label)| {
                    copy.get(&key).map(|value| CopyValue {
                        label,
                        value: value.clone(),
                    })
                })
                .collect(),
            hosts: credential.hosts,
            secrets: credential
                .secrets
                .into_iter()
                .map(|secret| AskedSecret {
                    name: secret.name,
                    label: secret.label,
                    masked: secret.masked,
                    pattern: secret.pattern,
                    set: false,
                    value: None,
                })
                .collect(),
            saved: false,
        }
    }
}

/// A value the person enters at the provider, beside its label there.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopyValue {
    pub label: String,
    pub value: String,
}

/// A secret a request asks for.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AskedSecret {
    pub name: String,
    /// What the provider calls it; the name when empty.
    #[serde(default)]
    pub label: String,
    /// Never shown once set.
    pub masked: bool,
    /// A regular expression a value must match.
    #[serde(default)]
    pub pattern: Option<String>,
    pub set: bool,
    /// Its value, when it is set and not masked.
    #[serde(default)]
    pub value: Option<String>,
}

/// Where paying a request goes next.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayResponse {
    /// Stripe Checkout, or none when an active subscription moved onto the
    /// plan in place.
    pub url: Option<String>,
}

/// One credential's values by secret name, at `PUT /requests/{code}/secrets`.
/// A secret of it already set may be left out. `hosts` replaces the hosts of
/// every secret of it, the ones left out included.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretValues {
    pub values: BTreeMap<String, String>,
    pub hosts: Vec<String>,
}

/// A sign-in request a device made, at `POST /requests/login`: RFC 8628's
/// device authorization response.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceAuthorization {
    /// What the device polls with. Never shown to the person.
    pub device_code: String,
    /// What the person enters at `verification_uri`, as `WDJB-MJHT`.
    pub user_code: String,
    pub verification_uri: String,
    /// The page that opens the request without typing its code.
    pub verification_uri_complete: String,
    /// Seconds the codes last.
    pub expires_in: i64,
    /// Seconds to wait between polls.
    pub interval: u64,
}

/// A device's poll, at `POST /requests/login/token`.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceTokenRequest {
    pub device_code: String,
}

/// The session an approved sign-in opens, answered to the first poll after
/// approval.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceToken {
    pub token: String,
}

/// RFC 8628's error codes a poll answers, in [`Error::Api`]'s `code`.
///
/// [`Error::Api`]: crate::client::Error::Api
pub mod device {
    /// Not approved or declined yet.
    pub const AUTHORIZATION_PENDING: &str = "authorization_pending";
    /// Polled sooner than `interval` after the last poll.
    pub const SLOW_DOWN: &str = "slow_down";
    /// The person declined it.
    pub const ACCESS_DENIED: &str = "access_denied";
    /// Past its expiry, or its token already answered.
    pub const EXPIRED_TOKEN: &str = "expired_token";
}
