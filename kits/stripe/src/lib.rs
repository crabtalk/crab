//! Stripe subscriptions as a kit: Checkout and the customer portal for the
//! user a call carries, and each user's plan, kept from Stripe's events. Its
//! tables are `stripe_customers` and `stripe_subscriptions`.

#![no_std]

extern crate alloc;

use alloc::string::String;
use berm_lang::{Schema, User};

mod api;
mod subscriber;
mod webhook;

pub use api::{Customer, Item, List, Price, Subscription};

/// An end user, as the Stripe customer they check out as.
pub struct Subscriber {
    pub user: User,
}

/// What a Stripe event is about.
#[derive(Schema)]
pub struct Data {
    pub object: Object,
}

/// The object of a Stripe event, as far as this kit reads it.
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
