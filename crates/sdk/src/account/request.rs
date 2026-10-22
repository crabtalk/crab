//! `/requests` — what an account asks a person to do, handed to them as a
//! short code: pay for a purchase, or set secrets of a service.

use crate::{account::org::PublicAccount, utils::text_enum};
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
}

text_enum!(RequestKind { Payment => "payment", Secret => "secret" });

/// What a payment request buys.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Purchase {
    /// A paid plan's subscription.
    Plan,
    /// The free trial of the plan that offers one.
    Trial,
    /// Credit, in an amount chosen at checkout.
    Credit,
}

text_enum!(Purchase { Plan => "plan", Trial => "trial", Credit => "credit" });

/// Where a request stands.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RequestStatus {
    /// Waiting on the person.
    Pending,
    /// Paid, the trial started, or the secrets set.
    Completed,
    /// The person declined it.
    Declined,
    /// Left pending past `expires_at`.
    Expired,
}

text_enum!(RequestStatus {
    Pending => "pending",
    Completed => "completed",
    Declined => "declined",
    Expired => "expired"
});

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
    /// Set secrets of one of the account's services, 1 to 20, each beside a
    /// guide to getting it.
    Secret {
        account: String,
        service: String,
        secrets: Vec<AskedSecret>,
        /// What it is for, in the asker's words, shown to the person.
        #[serde(default)]
        context: String,
    },
}

/// A request, as the person who opens its code sees it.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    /// What a person types at `/pay` or `/secrets`, as `WDJB-MJHT`.
    pub code: String,
    /// The page that opens it.
    pub url: String,
    pub kind: RequestKind,
    pub status: RequestStatus,
    /// The account it is for.
    pub account: PublicAccount,
    pub context: String,
    pub created_at: i64,
    pub expires_at: i64,
    /// What a payment request buys.
    pub purchase: Option<Purchase>,
    /// The plan a `plan` or `trial` purchase subscribes to.
    pub plan: Option<String>,
    /// The plan's monthly price in microdollars; none for credit, whose
    /// amount is chosen at checkout.
    pub price_micros: Option<i64>,
    /// Days a `trial` purchase is free for, before the plan's price.
    pub trial_days: Option<u32>,
    /// Paying ends the account's trial into the plan at once, charging the
    /// card the trial was started with; no Checkout follows.
    pub card_on_file: bool,
    /// The service a secret request sets secrets of.
    pub service: Option<String>,
    /// The secrets a secret request asks for, in order; never their values.
    #[serde(default)]
    pub secrets: Vec<AskedSecret>,
}

/// A secret a request asks for, and how to get it.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AskedSecret {
    pub name: String,
    /// Where and how the person gets its value, in the asker's words, shown
    /// beside its field.
    #[serde(default)]
    pub guide: String,
}

/// Where paying a request goes next.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayResponse {
    /// Stripe Checkout, or none when a trial ended into the plan at once.
    pub url: Option<String>,
}

/// A secret request's values by secret name, at `PUT /requests/{code}/secrets`:
/// one for each secret it asks for.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretValues {
    pub values: BTreeMap<String, String>,
}
