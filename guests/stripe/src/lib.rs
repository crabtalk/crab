//! Stripe subscriptions as a service: Checkout and the customer portal for the
//! user a call carries, and each user's plan, kept from Stripe's events.

#![cfg_attr(target_arch = "wasm32", no_std, no_main)]

extern crate alloc;

use alloc::string::String;
use berm_lang::Schema;

mod api;
mod utils;

#[berm_lang::program(
    readme = "README.md",
    database = "migrations",
    secrets = ["STRIPE_SECRET_KEY"],
    syscalls = {
        "berm.http.fetch" = ["api.stripe.com"],
    },
)]
mod tools {
    use crate::{Data, Session, StripeError, api, utils};
    use alloc::string::String;
    use berm_lang::Result;

    /// Opens a Stripe Checkout session subscribing the calling user to a
    /// plan, and answers the URL to send them to.
    #[public]
    #[callers(services)]
    pub fn checkout(
        /// The plan: the `lookup_key` of one of the product's Stripe prices.
        price: String,
        /// Where Checkout sends the user once they subscribe.
        success: String,
        /// Where Checkout sends the user if they leave without subscribing.
        cancel: String,
    ) -> Result<Session> {
        let user = utils::user()?;
        let price = api::price(&price)?;
        let customer = match utils::customer(&user)? {
            Some(customer) => customer,
            None => {
                let customer = api::customer(&user)?;
                berm_lang::rows::<()>(
                    "INSERT INTO customers (issuer, subject, customer) VALUES (?, ?, ?)",
                    &(&user.issuer, &user.subject, &customer),
                )?;
                customer
            }
        };
        api::checkout(&customer, &price, &success, &cancel)
    }

    /// Opens a Stripe customer portal session for the calling user, and
    /// answers the URL to send them to.
    #[public]
    #[callers(services)]
    pub fn portal(
        /// Where the portal sends the user back to.
        back: String,
    ) -> Result<Session> {
        let user = utils::user()?;
        let customer = utils::customer(&user)?.ok_or(StripeError::NoCustomer)?;
        api::portal(&customer, &back)
    }

    /// The calling user's plan: the `lookup_key` of the price their active or
    /// trialing subscription is on, or `null` without one.
    #[query]
    #[public]
    #[callers(services)]
    pub fn plan() -> Result<Option<String>> {
        let user = utils::user()?;
        let plans = berm_lang::rows::<(Option<String>,)>(
            "SELECT s.plan FROM subscriptions s
             JOIN customers c ON c.customer = s.customer
             WHERE c.issuer = ? AND c.subject = ? AND s.status IN ('active', 'trialing')
             ORDER BY s.id LIMIT 1",
            &(&user.issuer, &user.subject),
        )?;
        Ok(plans.into_iter().next().and_then(|(plan,)| plan))
    }

    /// Keeps the subscription a Stripe event names as Stripe answers it now.
    #[stripe]
    pub fn webhook(
        /// What the event is about.
        data: Data,
    ) -> Result<()> {
        let object = data.object;
        let id = match object.object.as_str() {
            "subscription" => object.id,
            "checkout.session" => match object.subscription {
                Some(id) => id,
                None => return Ok(()),
            },
            _ => return Ok(()),
        };
        let subscription = api::subscription(&id)?;
        let plan = subscription
            .items
            .data
            .into_iter()
            .next()
            .and_then(|item| item.price.lookup_key);
        berm_lang::rows::<()>(
            "INSERT INTO subscriptions (id, customer, status, plan) VALUES (?, ?, ?, ?)
             ON CONFLICT (id) DO UPDATE
             SET customer = excluded.customer, status = excluded.status, plan = excluded.plan",
            &(
                &subscription.id,
                &subscription.customer,
                &subscription.status,
                &plan,
            ),
        )?;
        Ok(())
    }
}

/// What a Stripe event is about.
#[derive(Schema)]
pub struct Data {
    pub object: Object,
}

/// The object of a Stripe event, as far as this service reads it.
#[derive(Schema)]
pub struct Object {
    pub id: String,
    /// Its kind, such as `checkout.session` or `subscription`.
    pub object: String,
    /// A Checkout session's subscription.
    pub subscription: Option<String>,
}

/// A page Stripe hosts.
#[derive(Schema)]
pub struct Session {
    /// Where to send the user.
    pub url: String,
}

/// Why a call is refused.
#[berm_lang::error_code]
pub enum StripeError {
    #[msg("this call carries no user")]
    NoUser,
    #[msg("no active price has that lookup key")]
    NoPrice,
    #[msg("the user has never checked out")]
    NoCustomer,
}
