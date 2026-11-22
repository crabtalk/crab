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
const SETUP: &str =
    "Grant a Stripe restricted key for api.stripe.com and call connect_stripe with its secret ID";

#[derive(Schema)]
pub struct Synced {
    pub checkout_sessions: u64,
    pub payment_intents: u64,
    pub renewal_invoices: u64,
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
        Self::granted_owner(&id)?;
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
        let checkout_sessions = self.import(
            "checkout/sessions",
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
            self.import("payment_intents", None, through, |row: &Record<Intent>| {
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
        let renewal_invoices = self.import(
            "invoices",
            Some("paid"),
            through,
            |row: &Record<Invoice>| {
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
            },
        )?;
        Ok(Synced {
            checkout_sessions,
            payment_intents,
            renewal_invoices,
        })
    }

    fn import<T: DeserializeOwned>(
        &self,
        kind: &str,
        status: Option<&str>,
        through: u64,
        mut keep: impl FnMut(&Record<T>) -> Result<bool>,
    ) -> Result<u64> {
        let cursor = caller_storage::rows::<(u64,)>(
            "SELECT created FROM cursors WHERE kind = ?1",
            &(kind,),
        )?
        .pop()
        .map_or(0, |(created,)| created);
        let auth = format!("Bearer {{secret:{}}}", self.0);
        let client = http::Client {
            name: "Stripe",
            headers: &[("Authorization", &auth), ("Stripe-Version", STRIPE_VERSION)],
        };
        let mut base = Url::parse(&format!("{STRIPE_API}/{kind}")).expect("valid Stripe endpoint");
        let mut query = base.query_pairs_mut();
        if let Some(status) = status {
            query.append_pair("status", status);
        }
        query
            .append_pair("created[gt]", &cursor.to_string())
            .append_pair("created[lte]", &through.to_string())
            .append_pair("limit", "25");
        drop(query);

        let mut latest = cursor;
        let mut after = None::<String>;
        let mut count = 0;
        loop {
            let mut url = base.clone();
            if let Some(after) = &after {
                url.query_pairs_mut().append_pair("starting_after", after);
            }
            let page: List<T> = client.json("GET", url.as_str(), b"")?;
            for row in &page.data {
                latest = latest.max(row.created);
                count += u64::from(keep(row)?);
            }
            if !page.has_more {
                break;
            }
            let next = page
                .data
                .last()
                .map(|row| &row.id)
                .filter(|next| Some(*next) != after.as_ref())
                .ok_or("Stripe pagination did not advance")?;
            after = Some(next.clone());
        }
        if latest > cursor {
            caller_storage::rows::<()>(
                "INSERT INTO cursors (kind, created) VALUES (?1, ?2)
                 ON CONFLICT (kind) DO UPDATE SET created = excluded.created",
                &(kind, latest),
            )?;
        }
        Ok(count)
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
