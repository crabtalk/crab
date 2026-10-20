//! Stripe's API, through `berm.http.fetch`, as the restricted key in
//! `STRIPE_SECRET_KEY`.

use crate::{Session, StripeError};
use alloc::{format, string::String, vec::Vec};
use berm_lang::{
    Result, User, http,
    serde::{Deserialize, de::DeserializeOwned},
    serde_json,
};

const API: &str = "https://api.stripe.com/v1";
const AUTHORIZATION: &str = "Bearer {secret:STRIPE_SECRET_KEY}";
/// The API version every request is read and answered in.
const VERSION: &str = "2025-03-31.basil";

/// A Stripe customer.
#[derive(Deserialize)]
#[serde(crate = "berm_lang::serde")]
pub struct Customer {
    /// Its id, `cus_…`.
    pub id: String,
}

/// A subscription, as far as this kit reads it.
#[derive(Deserialize)]
#[serde(crate = "berm_lang::serde")]
pub struct Subscription {
    pub id: String,
    /// The id of its customer.
    pub customer: String,
    pub status: String,
    pub items: List<Item>,
}

/// One of a subscription's items.
#[derive(Deserialize)]
#[serde(crate = "berm_lang::serde")]
pub struct Item {
    pub price: Price,
}

/// A price, as far as this kit reads it.
#[derive(Deserialize)]
#[serde(crate = "berm_lang::serde")]
pub struct Price {
    pub id: String,
    pub lookup_key: Option<String>,
}

/// A page of a Stripe list.
#[derive(Deserialize)]
#[serde(crate = "berm_lang::serde")]
pub struct List<T> {
    pub data: Vec<T>,
}

#[derive(Deserialize)]
#[serde(crate = "berm_lang::serde")]
struct Failure {
    error: Message,
}

#[derive(Deserialize)]
#[serde(crate = "berm_lang::serde")]
struct Message {
    #[serde(default)]
    message: String,
}

impl Price {
    /// The active price whose lookup key is `lookup_key`.
    pub fn find(lookup_key: &str) -> Result<Self> {
        let prices: List<Self> = send(
            "GET",
            "/prices",
            &[("lookup_keys[]", lookup_key), ("active", "true")],
        )?;
        Ok(prices.data.into_iter().next().ok_or(StripeError::NoPrice)?)
    }
}

impl Customer {
    /// A new Stripe customer with `user`'s issuer and subject in its metadata.
    /// Nothing records it as `user`'s.
    pub fn create(user: &User) -> Result<Self> {
        send(
            "POST",
            "/customers",
            &[
                ("metadata[issuer]", &user.issuer),
                ("metadata[subject]", &user.subject),
            ],
        )
    }

    /// A Checkout session subscribing this customer to `price`, returning
    /// them to `success` or `cancel`.
    pub fn checkout(&self, price: &Price, success: &str, cancel: &str) -> Result<Session> {
        send(
            "POST",
            "/checkout/sessions",
            &[
                ("mode", "subscription"),
                ("customer", &self.id),
                ("line_items[0][price]", &price.id),
                ("line_items[0][quantity]", "1"),
                ("success_url", success),
                ("cancel_url", cancel),
            ],
        )
    }

    /// A customer portal session for this customer, returning them to `back`.
    pub fn portal(&self, back: &str) -> Result<Session> {
        send(
            "POST",
            "/billing_portal/sessions",
            &[("customer", &self.id), ("return_url", back)],
        )
    }
}

impl Subscription {
    /// Subscription `id` as Stripe holds it now.
    pub fn retrieve(id: &str) -> Result<Self> {
        send("GET", &format!("/subscriptions/{}", http::encode(id)), &[])
    }

    /// The `lookup_key` of its first item's price.
    pub fn plan(&self) -> Option<&str> {
        self.items.data.first()?.price.lookup_key.as_deref()
    }
}

/// `fields` sent to `path`, in the query of a `GET` and the body otherwise,
/// and Stripe's answer read as `T`.
fn send<T: DeserializeOwned>(method: &str, path: &str, fields: &[(&str, &str)]) -> Result<T> {
    let form = http::form(fields);
    let (url, body) = match method {
        "GET" if !form.is_empty() => (format!("{API}{path}?{form}"), String::new()),
        "GET" => (format!("{API}{path}"), String::new()),
        _ => (format!("{API}{path}"), form),
    };
    let response = http::fetch(
        method,
        &url,
        &[
            ("Authorization", AUTHORIZATION),
            ("Stripe-Version", VERSION),
            ("Content-Type", "application/x-www-form-urlencoded"),
        ],
        body.as_bytes(),
    )?;
    if !(200..300).contains(&response.status) {
        let message = serde_json::from_slice::<Failure>(&response.body)
            .map(|failure| failure.error.message)
            .unwrap_or_default();
        return Err(format!("Stripe answered {}: {message}", response.status).into());
    }
    serde_json::from_slice(&response.body)
        .map_err(|_| String::from("Stripe answered what this service cannot read").into())
}
