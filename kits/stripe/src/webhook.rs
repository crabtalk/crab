//! Stripe's events, kept as the subscriptions they name.

use crate::{Data, Subscription};
use berm_lang::Result;

impl Data {
    /// Keeps the subscription this event names as Stripe answers it now: a
    /// subscription's own, or a Checkout session's. Any other event keeps
    /// nothing.
    pub fn keep(&self) -> Result<()> {
        let object = &self.object;
        let id = match (object.object.as_str(), &object.subscription) {
            ("subscription", _) => &object.id,
            ("checkout.session", Some(id)) => id,
            _ => return Ok(()),
        };
        Subscription::retrieve(id)?.keep()
    }
}

impl Subscription {
    /// Records this subscription as it reads now, over what was recorded.
    pub fn keep(&self) -> Result<()> {
        berm_lang::rows::<()>(
            "INSERT INTO stripe_subscriptions (id, customer, status, plan) VALUES (?, ?, ?, ?)
             ON CONFLICT (id) DO UPDATE
             SET customer = excluded.customer, status = excluded.status, plan = excluded.plan",
            &(&self.id, &self.customer, &self.status, self.plan()),
        )?;
        Ok(())
    }
}
