//! Billing, plan-catalog, and invoice wire types.

use crate::{account::org::Kind, services::price::Kind as PriceKind, system::meta::Rates};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A public plan catalog entry.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicPlan {
    /// Identifier used in `POST /orgs/{org}/billing/checkout?plan=<slug>`.
    pub slug: String,
    pub accounts: Vec<Kind>,
    /// Usage included each period, in USD micros at the list rates. It pays
    /// compute only and resets with the period.
    #[serde(default)]
    pub usage_micros: u64,
    /// `usage_micros` under its earlier name, sent for clients that require it.
    #[serde(default)]
    pub credit_micros: u64,
    /// Monthly gross price in microdollars — what the customer is charged.
    /// `null` is a plan that is not sold: a granted rung.
    pub price_micros: Option<i64>,
    /// The list rates `/meta/rates` serves, sent for clients that require them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rates: Option<Rates>,
    /// The largest overage allowance the plan permits, in USD micros.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overage_max_micros: Option<u64>,
    /// GitHub verification terms when the plan requires verification.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verification: Option<VerificationTerms>,
    /// The plan's storage and outbound data limits.
    pub included: Included,
    /// Bytes of memory the account's services may hold at once, their images
    /// and their calls running; `0` is no limit.
    pub memory: u64,
}

/// Minimum GitHub account age in days and stars across its own public repositories.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationTerms {
    pub days: u32,
    pub stars: u32,
}

/// Bytes held against the account's storage limit.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct StorageAllocation {
    pub databases: u64,
    pub objects: u64,
    /// Logs kept, as their nodes last sampled them.
    #[serde(default)]
    pub logs: u64,
}

/// An account's billing period: what its plan includes, what it used, and
/// what it was warned of.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Period {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage: Option<StorageAllocation>,
    /// The plan's slug.
    pub plan: String,
    /// Days since the Unix epoch, `until` excluded.
    pub since: i32,
    pub until: i32,
    /// When the period begins and ends, in Unix seconds, `ends_at` excluded.
    #[serde(default)]
    pub starts_at: i64,
    #[serde(default)]
    pub ends_at: i64,
    pub included: Included,
    /// `stored` includes database reservations, images and pending uploads.
    pub used: Usage,
    /// Legacy storage aggregate; not database allocation.
    pub stored: u64,
    /// What the plan includes this period, in USD micros; it pays compute only.
    #[serde(default)]
    pub included_micros: i64,
    /// What its services' compute cost this period at the rates, in USD micros.
    #[serde(default)]
    pub compute_micros: i64,
    /// `compute_micros` past `included_micros`.
    #[serde(default)]
    pub overage_micros: i64,
    /// Overage charged to the card and paid.
    #[serde(default)]
    pub collected_micros: i64,
    /// What `compute_micros` is made of, by unit, in USD micros, each rounded
    /// down; a unit costing nothing is left out.
    #[serde(default)]
    pub costs: BTreeMap<Unit, i64>,
    /// Each share of a limit reached, and an overage charge that failed.
    pub warnings: Vec<Warning>,
    /// Prices paid to other accounts since `since`, by operation, the most first.
    #[serde(default)]
    pub spent: Vec<Spent>,
}

/// What an account paid one operation's price.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Spent {
    #[serde(default)]
    pub kind: PriceKind,
    /// The service's owner, by handle, else by id.
    pub owner: String,
    pub service: String,
    pub tool: String,
    /// Priced calls or runs, including current holds.
    pub calls: u64,
    /// In USD micros.
    pub micros: i64,
}

/// One price an account paid, or had held and released.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Purchase {
    /// When it was held, in epoch seconds.
    pub at: i64,
    #[serde(default)]
    pub kind: PriceKind,
    /// The service's owner, by handle, else by id.
    pub owner: String,
    pub service: String,
    pub tool: String,
    /// The API key that made the call; `None` for a session.
    pub key: Option<String>,
    /// In USD micros.
    pub micros: i64,
    /// Whether the price was given back: the call did not end done.
    pub released: bool,
}

/// One UTC calendar month of [`Spent`], by operation, the most first.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpentMonth {
    /// `YYYY-MM`.
    pub month: String,
    pub spent: Vec<Spent>,
}

/// A share of a limit an account reached in a period.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Warning {
    pub unit: Unit,
    /// In percent.
    pub threshold: u32,
}

/// A metered unit: a field of [`Usage`], or `Owed` when an overage charge failed.
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
    Domains,
    /// Spelled `credit` on the wire, which released clients know.
    #[serde(rename = "credit")]
    Owed,
}

berm_api::text_enum!(Unit);

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

berm_api::text_enum!(SubscriptionStatus);

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

berm_api::text_enum!(InvoiceStatus);

/// What a plan includes besides Credit: limits summed over the
/// account's services.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Included {
    /// Bytes the account's services reserve for their databases together, and
    /// their logs. A deploy that would reserve past it is refused.
    pub stored: u64,
    /// Bytes sent off the nodes: answers to outside callers and requests to
    /// other sites, rotating traffic left out. Past it the account's services
    /// take no calls until the period ends; `0` is no cap.
    pub sent: u64,
}

/// What an account's services used in a period, summed over them.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Usage {
    pub runs: u64,
    /// CPU microseconds past each run's first
    /// [`Metering::included_cpu`](crate::system::meta::Metering::included_cpu).
    pub cpu: u64,
    /// Rows read.
    pub read: u64,
    /// Rows written.
    pub written: u64,
    /// MiB-microseconds of memory budget the account's calls held, each from
    /// its start to its return, waiting included.
    pub held: u64,
    /// Bytes held against [`Included::stored`].
    pub stored: u64,
    /// Bytes counted against [`Included::sent`].
    pub sent: u64,
    /// Days custom domains were held, summed over them, each day begun counted.
    pub domains: u64,
}

/// A person's verification, which lets an account on Free deploy.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Verification {
    /// When it was verified, in Unix seconds; `None` until it is.
    pub verified_at: Option<i64>,
    /// Why verifying it now fails; `None` when verified or when it would pass.
    pub reason: Option<String>,
    /// The person signs in with GitHub.
    #[serde(default)]
    pub github: bool,
}

/// A plan subscribed: the Checkout URL to pay at, or `None` when an active
/// subscription moved onto the plan in place.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscribeResponse {
    pub url: Option<String>,
}

/// An account's prepaid balance and auto-refill, in USD micros.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Credit {
    /// Prepaid: what purchases and refills have left. It pays priced tools and
    /// rotating traffic.
    pub micros: i64,
    /// What of `micros` expires within 30 days.
    #[serde(default)]
    pub expiring_micros: i64,
    /// When a held account's services are retired, in Unix seconds.
    #[serde(default)]
    pub recovery_until: Option<i64>,
    /// The most refills may deposit in a UTC calendar month; `0` is off.
    pub refill_cap_micros: i64,
    /// What one refill buys; `0` when never set.
    pub refill_micros: i64,
    /// The prepaid balance below which a refill is charged.
    #[serde(default = "RefillRequest::below")]
    pub refill_below_micros: i64,
}

/// Where an account's credit came from and went.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreditHistory {
    /// A UTC calendar month each, newest first; only months credit moved in.
    pub months: Vec<CreditMonth>,
    /// The purchases and refills still holding credit, soonest to expire
    /// first.
    pub grants: Vec<Grant>,
}

/// One month of [`CreditHistory`], in USD micros, each figure positive.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreditMonth {
    /// `YYYY-MM`.
    pub month: String,
    /// Credit issued.
    pub added: i64,
    /// Metered execution.
    pub overage: i64,
    /// Other accounts' prices, less released holds.
    pub prices: i64,
    pub expired: i64,
    /// The balance when the month ended, or now for the current month.
    pub balance: i64,
}

/// One credit grant.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Grant {
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub reference: Option<String>,
    pub id: i64,
    /// What is left of it, in USD micros.
    pub remaining: i64,
    /// Unix seconds.
    pub created_at: i64,
    /// Unix seconds.
    pub expires_at: i64,
}

/// What an account's priced operations earned, a UTC calendar month each, newest
/// first, and what was paid out of it.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Earnings {
    pub months: Vec<EarnedMonth>,
    /// Whether the account has a Stripe Connect account.
    pub connected: bool,
    /// Whether Stripe lets that account receive payouts.
    pub payouts_enabled: bool,
    /// Closed months' payable balances, each counted from zero, less transfers and fees.
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

berm_api::text_enum!(PayoutStatus);

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

/// One priced operation's part of an [`EarnedMonth`].
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EarnedTool {
    #[serde(default)]
    pub kind: PriceKind,
    pub service: String,
    pub tool: String,
    #[serde(flatten)]
    pub earned: Earned,
}

/// What priced calls earned, in USD micros.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Earned {
    /// Completed tool calls or workflow runs.
    pub calls: u64,
    /// Their prices, summed.
    pub revenue: i64,
    /// The platform's cut of `revenue`.
    pub cut: i64,
    /// Managed execution and dependency charges attributed to these sales.
    pub compute: i64,
    /// `revenue` less `cut` and `compute`; below zero when compute is larger.
    pub net: i64,
    /// Revenue less commission and costs deducted from earnings, before payout fees.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payable: Option<i64>,
}

impl Earned {
    pub fn new(calls: u64, revenue: i64, cut: i64, compute: i64, deducted: i64) -> Self {
        Self {
            calls,
            revenue,
            cut,
            compute,
            net: revenue - cut - compute,
            payable: Some(revenue - cut - deducted),
        }
    }

    /// Legacy responses omit this balance and deduct their costs from earnings.
    pub fn payable(&self) -> i64 {
        self.payable.unwrap_or(self.net)
    }
}

impl std::ops::AddAssign for Earned {
    fn add_assign(&mut self, other: Self) {
        *self = Self {
            calls: self.calls + other.calls,
            revenue: self.revenue + other.revenue,
            cut: self.cut + other.cut,
            compute: self.compute + other.compute,
            net: self.net + other.net,
            payable: Some(self.payable() + other.payable()),
        };
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
    /// The prepaid balance below which a refill is charged.
    #[serde(default = "RefillRequest::below")]
    pub below_micros: i64,
}

impl RefillRequest {
    /// The balance a refill is charged below when none is set: $5.
    pub fn below() -> i64 {
        5_000_000
    }
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

berm_api::text_enum!(PortalFlow);

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

/// One row of a customer's invoice history, a subset of the Stripe invoice
/// object. The document is Stripe's generated `invoice_pdf`.
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
