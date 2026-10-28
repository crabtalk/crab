//! Billing, plan-catalog, and invoice wire types.

use crate::{account::org::Kind, utils::text_enum};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// One plan as a client renders it: what the plan promises, never what keeping
/// the promise costs us. Every field is a limit the caller trips directly and
/// can act on, so `null` always means the plan carries no such value — no cap,
/// no restriction, no price — and never that the answer lives somewhere else.
/// Labels and copy are the frontend's: cloud serves the slug.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicPlan {
    /// Identifier used in `POST /orgs/{org}/billing/checkout?plan=<slug>`.
    pub slug: String,
    /// Which kind of account this plan is for, or `null` for either. Checkout
    /// answers `400` on a mismatch, so a client partitions its pricing page on
    /// this rather than discovering it at the buy button.
    pub kind: Option<Kind>,
    /// Monthly gross price in microdollars — what the customer is charged.
    /// `null` is a plan that is not sold, which is the whole signal for a
    /// granted rung.
    pub price_micros: Option<i64>,
    /// What the plan includes each billing period.
    pub included: Included,
    /// Bytes of memory the account's services may hold at once, their images
    /// and their calls running; `0` is no limit.
    pub memory: u64,
}

/// An account's billing period: what its plan includes, what it used, and
/// what it was warned of.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Period {
    /// The plan's slug: comped, else bought, else the one the account's kind
    /// is granted.
    pub plan: String,
    /// Days since the Unix epoch, `until` excluded.
    pub since: i32,
    pub until: i32,
    pub included: Included,
    /// In the units of `included`; `stored` is the bytes its services reserve.
    pub used: Included,
    /// Bytes its placed services keep, as each was last sampled.
    pub stored: u64,
    /// What its services used since `since` past the included amounts costs
    /// at the rates: what credit pays, in USD micros.
    #[serde(default)]
    pub charge: i64,
    /// What `charge` is made of, by unit past its included amount, in USD
    /// micros, each rounded down; a unit within it is left out.
    #[serde(default)]
    pub costs: BTreeMap<Unit, i64>,
    /// Credit drawn since `since`, in USD micros.
    #[serde(default)]
    pub drawn: i64,
    /// Each share of an included amount reached, and running out of credit.
    pub warnings: Vec<Warning>,
    /// What the account paid other accounts' tools' prices since `since`, by
    /// tool, the most first.
    #[serde(default)]
    pub spent: Vec<Spent>,
}

/// What an account paid one tool's price.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Spent {
    /// The service's owner, by handle, else by id.
    pub owner: String,
    pub service: String,
    pub tool: String,
    /// Calls charged; a call is charged from its start and returned when it
    /// fails.
    pub calls: u64,
    /// In USD micros.
    pub micros: i64,
}

/// A share of an included amount an account reached in a period.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Warning {
    pub unit: Unit,
    /// In percent.
    pub threshold: u32,
}

/// What a warning is about: a field of [`Included`], or `credit` when credit
/// ran out.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Unit {
    Runs,
    Cpu,
    Read,
    Written,
    Held,
    Stored,
    Sent,
    Credit,
}

text_enum!(Unit {
    Runs => "runs",
    Cpu => "cpu",
    Read => "read",
    Written => "written",
    Held => "held",
    Stored => "stored",
    Sent => "sent",
    Credit => "credit",
});

/// A Stripe subscription's status, as Stripe spells it.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubscriptionStatus {
    Incomplete,
    IncompleteExpired,
    Trialing,
    Active,
    PastDue,
    Canceled,
    Unpaid,
    Paused,
}

text_enum!(SubscriptionStatus {
    Incomplete => "incomplete",
    IncompleteExpired => "incomplete_expired",
    Trialing => "trialing",
    Active => "active",
    PastDue => "past_due",
    Canceled => "canceled",
    Unpaid => "unpaid",
    Paused => "paused",
});

impl SubscriptionStatus {
    /// Whether the subscription never leaves this status, and so can no longer
    /// move money.
    pub fn terminal(&self) -> bool {
        matches!(self, Self::Canceled | Self::IncompleteExpired)
    }
}

/// A Stripe invoice's status, as Stripe spells it.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InvoiceStatus {
    Draft,
    Open,
    Paid,
    Uncollectible,
    Void,
}

text_enum!(InvoiceStatus {
    Draft => "draft",
    Open => "open",
    Paid => "paid",
    Uncollectible => "uncollectible",
    Void => "void",
});

/// What a plan includes each billing period, summed over the account's
/// services.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Included {
    pub runs: u64,
    /// CPU microseconds past each run's first
    /// [`INCLUDED_CPU`](crate::system::meta::INCLUDED_CPU).
    pub cpu: u64,
    /// Rows read.
    pub read: u64,
    /// Rows written.
    pub written: u64,
    /// MiB-microseconds of memory budget the account's calls held, each from
    /// its start to its return, waiting included.
    #[serde(default)]
    pub held: u64,
    /// Bytes the account's services reserve for their databases together. A
    /// deploy that would reserve past it is refused.
    pub stored: u64,
    /// Bytes sent off the nodes: answers to outside callers and requests to
    /// other sites. Past it the account's services take no calls until the
    /// period ends; `0` is no cap.
    pub sent: u64,
}

/// A person's verification, which lets an account on Hobby deploy.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Verification {
    /// When it was verified, in Unix seconds; `None` until it is.
    pub verified_at: Option<i64>,
    /// Why verifying it now fails; `None` when verified or when it would pass.
    pub reason: Option<String>,
}

/// A plan subscribed: the Checkout URL to pay at, or `None` when an active
/// subscription moved onto the plan in place.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscribeResponse {
    pub url: Option<String>,
}

/// An account's credit, in USD micros. All of it was bought; it never
/// expires.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Credit {
    pub micros: i64,
    /// The most refills may deposit in a UTC calendar month; `0` is off.
    pub refill_cap_micros: i64,
    /// What one refill buys; `0` when never set.
    pub refill_micros: i64,
}

/// What an account's priced tools earned, a UTC calendar month each, newest
/// first, and what was paid out of it.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Earnings {
    pub months: Vec<EarnedMonth>,
    /// Whether the account has a Stripe Connect account.
    pub connected: bool,
    /// Whether Stripe lets that account receive payouts.
    pub payouts_enabled: bool,
    /// What closed months netted, each counted from `0`, less every payout
    /// and its fees, in USD micros.
    pub owed: i64,
    /// Newest first.
    pub payouts: Vec<Payout>,
}

/// One transfer of earnings to the account's Stripe Connect account.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Payout {
    /// `YYYY-MM`, the closed month it was made for.
    pub month: String,
    /// What was transferred, in USD micros.
    pub amount: i64,
    /// Connect's fees, taken from what was owed, in USD micros.
    pub fees: i64,
    pub status: PayoutStatus,
    /// Unix seconds.
    pub created_at: i64,
}

/// Where a [`Payout`] stands.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PayoutStatus {
    /// Recorded, its transfer not yet made; retried until it is.
    Pending,
    /// Transferred.
    Paid,
}

text_enum!(PayoutStatus {
    Pending => "pending",
    Paid => "paid",
});

/// A Stripe-hosted page to open.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StripeLink {
    pub url: String,
}

/// One month of [`Earnings`]: the sum, and each tool's part of it.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EarnedMonth {
    /// `YYYY-MM`.
    pub month: String,
    #[serde(flatten)]
    pub earned: Earned,
    /// By service then tool.
    pub tools: Vec<EarnedTool>,
}

/// One tool's part of an [`EarnedMonth`].
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EarnedTool {
    pub service: String,
    pub tool: String,
    #[serde(flatten)]
    pub earned: Earned,
}

/// What priced calls earned, in USD micros.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Earned {
    /// Calls charged.
    pub calls: u64,
    /// Their prices, summed.
    pub revenue: i64,
    /// The platform's cut of `revenue`.
    pub cut: i64,
    /// What every priced call used, failed ones included, at the published
    /// rates.
    pub compute: i64,
    /// `revenue` less `cut` and `compute`; below zero when compute is larger.
    pub net: i64,
}

impl Earned {
    pub fn new(calls: u64, revenue: i64, cut: i64, compute: i64) -> Self {
        Self {
            calls,
            revenue,
            cut,
            compute,
            net: revenue - cut - compute,
        }
    }
}

impl std::ops::AddAssign for Earned {
    fn add_assign(&mut self, other: Self) {
        *self = Self::new(
            self.calls + other.calls,
            self.revenue + other.revenue,
            self.cut + other.cut,
            self.compute + other.compute,
        );
    }
}

/// The body of `PUT …/billing/refill`.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefillRequest {
    /// The most refills may deposit in a UTC calendar month; `0` turns refill
    /// off.
    pub cap_micros: i64,
    /// What one refill buys, $20 or more. Ignored when
    /// `cap_micros` is `0`.
    #[serde(default)]
    pub amount_micros: i64,
}

/// Which single action the portal opens on.
///
/// Omitted, the portal opens on its home page and the customer navigates. Named,
/// it opens on that one form with the rest of the portal hidden and returns as
/// soon as the action is done. These are the flows the deployment's portal
/// configuration enables.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PortalFlow {
    /// Add a card and make it the one that pays.
    PaymentMethodUpdate,
    /// Cancel the active subscription, effective at the end of the period
    /// already paid for.
    SubscriptionCancel,
}

impl PortalFlow {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PaymentMethodUpdate => "payment_method_update",
            Self::SubscriptionCancel => "subscription_cancel",
        }
    }
}

/// The card a customer's subscription charges.
///
/// Read to render "Visa •••• 4242, expires 04/29" beside the button that
/// changes it. `null` in place of this object is an account with nothing on
/// file.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentMethod {
    /// Card network, lowercase as Stripe spells it: `visa`, `mastercard`,
    /// `amex`, …
    pub brand: String,
    /// Last four digits, the only part of the number that ever leaves Stripe.
    pub last4: String,
    /// Expiry month, 1–12.
    pub exp_month: i64,
    pub exp_year: i64,
}

/// One row of a customer's invoice history. Shape mirrors the Stripe invoice
/// object; we surface only what a "download my invoices" list needs and lean
/// on Stripe's generated `invoice_pdf` rather than rendering documents
/// ourselves.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Invoice {
    pub id: String,
    /// Human-facing invoice number (e.g. `"A1B2C3-0001"`). `None` for drafts.
    pub number: Option<String>,
    /// Unix epoch (seconds) the invoice was created.
    pub created: i64,
    pub status: InvoiceStatus,
    /// Amount paid in the smallest currency unit (cents for USD).
    pub amount_paid: i64,
    pub currency: String,
    /// Direct PDF download link. `None` for drafts.
    pub invoice_pdf: Option<String>,
    /// Stripe-hosted invoice page. `None` for drafts.
    pub hosted_invoice_url: Option<String>,
}
