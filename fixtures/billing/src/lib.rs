//! The reference subscriptions service: Stripe Checkout, the customer portal
//! and each user's plan through `crab-stripe`, and its webhook.

#![cfg_attr(target_arch = "wasm32", no_std, no_main)]

extern crate alloc;

#[berm_lang::program(uses = [stripe])]
mod tools {
    use alloc::string::String;
    use berm_lang::Result;
    use stripe::{Data, Session, Subscriber};

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
        Subscriber::calling()?.checkout(&price, &success, &cancel)
    }

    /// Opens a Stripe customer portal session for the calling user, and
    /// answers the URL to send them to.
    #[public]
    #[callers(services)]
    pub fn portal(
        /// Where the portal sends the user back to.
        back: String,
    ) -> Result<Session> {
        Subscriber::calling()?.portal(&back)
    }

    /// The calling user's plan: the `lookup_key` of the price their active or
    /// trialing subscription is on, or `null` without one.
    #[query]
    #[public]
    #[callers(services)]
    pub fn plan() -> Result<Option<String>> {
        Subscriber::calling()?.plan()
    }

    /// Keeps the subscription a Stripe event names as Stripe answers it now.
    #[stripe]
    pub fn webhook(
        /// What the event is about.
        data: Data,
    ) -> Result<()> {
        data.keep()
    }
}
