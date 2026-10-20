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

/// A subscription, as far as this service reads it.
#[derive(Deserialize)]
#[serde(crate = "berm_lang::serde")]
pub struct Subscription {
    pub id: String,
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
struct Created {
    id: String,
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

/// The id of the active price whose lookup key is `lookup_key`.
pub fn price(lookup_key: &str) -> Result<String> {
    let prices: List<Price> = send(
        "GET",
        "/prices",
        &[("lookup_keys[]", lookup_key), ("active", "true")],
    )?;
    let price = prices.data.into_iter().next().ok_or(StripeError::NoPrice)?;
    Ok(price.id)
}

/// A new customer for `user`, by its id.
pub fn customer(user: &User) -> Result<String> {
    let created: Created = send(
        "POST",
        "/customers",
        &[
            ("metadata[issuer]", &user.issuer),
            ("metadata[subject]", &user.subject),
        ],
    )?;
    Ok(created.id)
}

/// A Checkout session subscribing `customer` to `price`, returning them to
/// `success` or `cancel`.
pub fn checkout(customer: &str, price: &str, success: &str, cancel: &str) -> Result<Session> {
    send(
        "POST",
        "/checkout/sessions",
        &[
            ("mode", "subscription"),
            ("customer", customer),
            ("line_items[0][price]", price),
            ("line_items[0][quantity]", "1"),
            ("success_url", success),
            ("cancel_url", cancel),
        ],
    )
}

/// A customer portal session for `customer`, returning them to `back`.
pub fn portal(customer: &str, back: &str) -> Result<Session> {
    send(
        "POST",
        "/billing_portal/sessions",
        &[("customer", customer), ("return_url", back)],
    )
}

/// The subscription `id` as Stripe holds it now.
pub fn subscription(id: &str) -> Result<Subscription> {
    send("GET", &format!("/subscriptions/{}", http::encode(id)), &[])
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
