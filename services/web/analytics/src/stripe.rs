use crate::account::Account;
use alloc::{
    format,
    string::{String, ToString},
    vec::Vec,
};
use berm_lang::{
    Result, Schema,
    abi::secrets::Id,
    caller_storage, deny, http, key, now, secret,
    serde::{Deserialize, de::DeserializeOwned},
    service,
};
use url::Url;

const STRIPE_HOST: &str = "api.stripe.com";
const STRIPE_API: &str = "https://api.stripe.com/v1";
// This version exposes the payment intent on an invoice.
const STRIPE_VERSION: &str = "2024-10-28.acacia";
const SECRET_KEY: &str = "stripe_secret_id";
const CHECKOUT_SESSIONS: &str = "checkout/sessions";
const PAYMENT_INTENTS: &str = "payment_intents";
const INVOICES: &str = "invoices";
const WINDOW_SECS: u64 = 365 * 24 * 60 * 60;
const REQUESTS_PER_ENDPOINT: usize = 6;
const PAGES_PER_WINDOW: usize = 2;
const PAGE_TIMEOUT_MS: u64 = 2_500;
const SETUP: &str =
    "Grant a Stripe restricted key for api.stripe.com and call connect_stripe with its secret ID";

#[derive(Schema, Default)]
pub struct Synced {
    pub checkout_sessions: u64,
    pub payment_intents: u64,
    pub renewal_invoices: u64,
    /// Oldest completed Stripe endpoint cursor, in Unix seconds.
    pub synced_through: u64,
    pub has_more: bool,
    pub error: Option<String>,
}

#[derive(Default)]
struct Imported {
    count: u64,
    error: Option<String>,
}

#[derive(Deserialize)]
#[serde(crate = "berm_lang::serde")]
struct List<T> {
    data: Vec<Record<T>>,
    has_more: bool,
}

#[derive(Deserialize)]
#[serde(crate = "berm_lang::serde")]
struct Record<T> {
    id: String,
    created: u64,
    customer: Option<String>,
    #[serde(flatten)]
    fields: T,
}

#[derive(Deserialize, Default)]
#[serde(crate = "berm_lang::serde")]
struct Metadata {
    crabtalk_visitor_id: Option<String>,
    crabtalk_session_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(crate = "berm_lang::serde")]
struct Checkout {
    metadata: Option<Metadata>,
}

#[derive(Deserialize)]
#[serde(crate = "berm_lang::serde")]
struct Intent {
    status: String,
    amount_received: u64,
    currency: String,
    #[serde(default)]
    metadata: Metadata,
}

#[derive(Deserialize)]
#[serde(crate = "berm_lang::serde")]
struct Invoice {
    billing_reason: Option<String>,
    payment_intent: Option<String>,
    amount_paid: u64,
    currency: String,
    status_transitions: StatusTransitions,
}

#[derive(Deserialize)]
#[serde(crate = "berm_lang::serde")]
struct StatusTransitions {
    paid_at: Option<u64>,
}

struct Payment<'a> {
    id: &'a str,
    customer: Option<&'a str>,
    visitor: Option<&'a str>,
    session: Option<&'a str>,
    amount: u64,
    currency: &'a str,
    at: u64,
}

pub(crate) struct Stripe(String);

impl Stripe {
    pub fn connect(account: &Account, id: &str) -> Result<()> {
        let owner = Self::granted_owner(id)?;
        let address = service()?.address;
        let deployment_owner = address.rsplit_once('/').map_or("", |(owner, _)| owner);
        // The host permits deployment-owned grants across callers, and an API key
        // can be bound to an account other than its creator's.
        if owner == deployment_owner && (owner != account.member || key::id()?.is_some()) {
            return Err(deny(SETUP));
        }
        caller_storage::set(SECRET_KEY, id)?;
        Ok(())
    }

    pub fn connected() -> Result<Option<Self>> {
        let Some(id) = caller_storage::get::<String>(SECRET_KEY)? else {
            return Ok(None);
        };
        Ok(Some(Self(id)))
    }

    pub fn required() -> Result<Self> {
        Self::connected()?.ok_or_else(|| deny(SETUP))
    }

    fn granted_owner(id: &str) -> Result<String> {
        Id::try_from(id)?;
        secret::owner(id, STRIPE_HOST)?.ok_or_else(|| deny(SETUP))
    }

    pub fn sync(&self) -> Result<Synced> {
        let through = (now() / 1000).saturating_sub(1);
        let synced_through = Self::synced_through()?;
        if synced_through >= through {
            return Ok(Synced {
                synced_through,
                ..Synced::default()
            });
        }
        let error = match secret::owner(&self.0, STRIPE_HOST) {
            Ok(Some(_)) => None,
            Ok(None) => Some(String::from(SETUP)),
            Err(error) => Some(error.to_string()),
        };
        if error.is_some() {
            return Ok(Synced {
                synced_through,
                has_more: true,
                error,
                ..Synced::default()
            });
        }

        let checkout_sessions = self.import(
            CHECKOUT_SESSIONS,
            Some("complete"),
            through,
            |row: &Record<Checkout>| {
                let (Some(customer), Some(visitor)) = (
                    &row.customer,
                    row.fields.metadata.as_ref().and_then(Metadata::visitor),
                ) else {
                    return Ok(false);
                };
                caller_storage::rows::<()>(
                    "INSERT OR IGNORE INTO customers (id, visitor) VALUES (?1, ?2)",
                    &(customer, visitor),
                )?;
                Ok(true)
            },
        )?;
        let payment_intents =
            self.import(PAYMENT_INTENTS, None, through, |row: &Record<Intent>| {
                let intent = &row.fields;
                if intent.status != "succeeded" {
                    return Ok(false);
                }
                Payment {
                    id: &row.id,
                    customer: row.customer.as_deref(),
                    visitor: intent.metadata.visitor(),
                    session: intent
                        .metadata
                        .crabtalk_session_id
                        .as_deref()
                        .filter(|id| !id.is_empty()),
                    amount: intent.amount_received,
                    currency: &intent.currency,
                    at: row.created,
                }
                .save()?;
                Ok(true)
            })?;
        let renewal_invoices =
            self.import(INVOICES, Some("paid"), through, |row: &Record<Invoice>| {
                let invoice = &row.fields;
                if invoice.billing_reason.as_deref() != Some("subscription_cycle")
                    || invoice.amount_paid == 0
                {
                    return Ok(false);
                }
                Payment {
                    id: invoice.payment_intent.as_deref().unwrap_or(&row.id),
                    customer: row.customer.as_deref(),
                    visitor: None,
                    session: None,
                    amount: invoice.amount_paid,
                    currency: &invoice.currency,
                    at: invoice.status_transitions.paid_at.unwrap_or(row.created),
                }
                .save()?;
                Ok(true)
            })?;
        let synced_through = Self::synced_through()?;
        Ok(Synced {
            checkout_sessions: checkout_sessions.count,
            payment_intents: payment_intents.count,
            renewal_invoices: renewal_invoices.count,
            synced_through,
            has_more: synced_through < through,
            error: checkout_sessions
                .error
                .or(payment_intents.error)
                .or(renewal_invoices.error),
        })
    }

    fn synced_through() -> Result<u64> {
        let rows = caller_storage::rows::<(u64,)>(
            "SELECT CASE WHEN count(*) = 3 THEN min(created) ELSE 0 END
             FROM cursors WHERE kind IN (?1, ?2, ?3)",
            &(CHECKOUT_SESSIONS, PAYMENT_INTENTS, INVOICES),
        )?;
        Ok(rows.first().map_or(0, |(created,)| *created))
    }

    fn import<T: DeserializeOwned>(
        &self,
        kind: &str,
        status: Option<&str>,
        through: u64,
        mut keep: impl FnMut(&Record<T>) -> Result<bool>,
    ) -> Result<Imported> {
        let mut cursor = caller_storage::rows::<(u64,)>(
            "SELECT created FROM cursors WHERE kind = ?1",
            &(kind,),
        )?
        .pop()
        .map_or(0, |(created,)| created);
        if cursor >= through {
            return Ok(Imported::default());
        }

        let auth = format!("Bearer {{secret:{}}}", self.0);
        let headers = &[
            ("Authorization", auth.as_str()),
            ("Stripe-Version", STRIPE_VERSION),
        ];
        let mut base = Url::parse(&format!("{STRIPE_API}/{kind}")).expect("valid Stripe endpoint");
        if let Some(status) = status {
            base.query_pairs_mut().append_pair("status", status);
        }

        // Remember reductions when a dense range takes several calls to split.
        let key = format!("stripe_window/{kind}");
        let previous_width = caller_storage::get::<u64>(&key)?.unwrap_or(WINDOW_SECS);
        let mut width = previous_width;
        let mut requests_left = REQUESTS_PER_ENDPOINT;
        let mut imported = Imported::default();
        while cursor < through && requests_left >= PAGES_PER_WINDOW {
            let upper = (cursor + width).min(through);
            match Self::window(&base, headers, cursor, upper, &mut requests_left) {
                Ok(Some(records)) => {
                    if records.is_empty() {
                        width = (width * 2).min(WINDOW_SECS);
                    }
                    for record in &records {
                        imported.count += u64::from(keep(record)?);
                    }
                    cursor = upper;
                    caller_storage::rows::<()>(
                        "INSERT INTO cursors (kind, created) VALUES (?1, ?2)
                         ON CONFLICT (kind) DO UPDATE SET created = excluded.created",
                        &(kind, cursor),
                    )?;
                }
                Ok(None) if upper - cursor > 1 => width = (upper - cursor) / 2,
                Ok(None) => {
                    imported.error = Some(format!(
                        "Stripe {kind} has more than {PAGES_PER_WINDOW} pages in one second at {upper}"
                    ));
                    break;
                }
                Err(error) => {
                    imported.error = Some(format!("Stripe {kind}: {error}"));
                    break;
                }
            }
        }
        if width != previous_width {
            caller_storage::set(&key, &width)?;
        }
        Ok(imported)
    }

    fn window<T: DeserializeOwned>(
        base: &Url,
        headers: &[(&str, &str)],
        cursor: u64,
        upper: u64,
        requests_left: &mut usize,
    ) -> Result<Option<Vec<Record<T>>>> {
        let mut range = base.clone();
        range
            .query_pairs_mut()
            .append_pair("created[gt]", &cursor.to_string())
            .append_pair("created[lte]", &upper.to_string())
            .append_pair("limit", "100");

        let mut url = range.clone();
        let mut records = Vec::<Record<T>>::new();
        for _ in 0..PAGES_PER_WINDOW {
            *requests_left -= 1;
            let response = http::fetch_all(&[http::Request {
                method: "GET",
                url: url.as_str(),
                headers,
                body: b"",
                timeout: Some(PAGE_TIMEOUT_MS),
                limit: None,
            }])?
            .pop()
            .ok_or("Missing response")??;
            if !response.is_success() {
                return Err(response.failure("HTTP"));
            }
            let page: List<T> = response.read("Stripe")?;
            let next = page.data.last().map(|record| &record.id);
            if page.has_more && (next.is_none() || next == records.last().map(|record| &record.id))
            {
                return Err("Pagination did not advance".into());
            }
            if let Some(next) = next {
                url = range.clone();
                url.query_pairs_mut().append_pair("starting_after", next);
            }
            records.extend(page.data);
            if !page.has_more {
                return Ok(Some(records));
            }
        }
        // Stripe lists newest first. Save a range only after its last page.
        Ok(None)
    }
}

impl Metadata {
    fn visitor(&self) -> Option<&str> {
        self.crabtalk_visitor_id
            .as_deref()
            .filter(|id| !id.is_empty())
    }
}

impl Payment<'_> {
    fn save(&self) -> Result<()> {
        caller_storage::rows::<()>(
            "INSERT INTO payments (id, customer, visitor, session, amount, currency, at)
             VALUES (?1, ?2, COALESCE(?3, (SELECT visitor FROM customers WHERE id = ?2)), ?4, ?5, ?6, ?7)
             ON CONFLICT (id) DO UPDATE SET
               customer = COALESCE(excluded.customer, payments.customer),
               visitor = COALESCE(?3, payments.visitor, excluded.visitor),
               session = COALESCE(excluded.session, payments.session),
               amount = excluded.amount, currency = excluded.currency, at = MAX(payments.at, excluded.at)",
            &(self.id, self.customer, self.visitor, self.session, self.amount, self.currency, self.at),
        )?;
        Ok(())
    }
}
