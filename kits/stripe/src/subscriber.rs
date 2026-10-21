//! The end user a call carries, as a Stripe customer and their plan.

use crate::{Customer, Price, Session, StripeError, Subscriber};
use alloc::string::String;
use berm_lang::Result;

impl Subscriber {
    /// The end user the call carries.
    pub fn calling() -> Result<Self> {
        let user = berm_lang::user()?.ok_or(StripeError::NoUser)?;
        Ok(Self { user })
    }

    /// Their Stripe customer, once they have checked out.
    pub fn customer(&self) -> Result<Option<Customer>> {
        let found = berm_lang::rows::<(String,)>(
            "SELECT customer FROM stripe_customers WHERE issuer = ? AND subject = ?",
            &(&self.user.issuer, &self.user.subject),
        )?;
        Ok(found.into_iter().next().map(|(id,)| Customer { id }))
    }

    /// A Checkout session subscribing them to the price whose lookup key is
    /// `plan`, making and recording their customer on their first checkout.
    pub fn checkout(&self, plan: &str, success: &str, cancel: &str) -> Result<Session> {
        let price = Price::find(plan)?;
        let customer = match self.customer()? {
            Some(customer) => customer,
            None => {
                let customer = Customer::create(&self.user)?;
                berm_lang::rows::<()>(
                    "INSERT INTO stripe_customers (issuer, subject, customer) VALUES (?, ?, ?)",
                    &(&self.user.issuer, &self.user.subject, &customer.id),
                )?;
                customer
            }
        };
        customer.checkout(&price, success, cancel)
    }

    /// A customer portal session for them, returning them to `back`.
    pub fn portal(&self, back: &str) -> Result<Session> {
        let customer = self.customer()?.ok_or(StripeError::NoCustomer)?;
        customer.portal(back)
    }

    /// The `lookup_key` of the price their active or trialing subscription is
    /// on.
    pub fn plan(&self) -> Result<Option<String>> {
        let plans = berm_lang::rows::<(Option<String>,)>(
            "SELECT s.plan FROM stripe_subscriptions s
             JOIN stripe_customers c ON c.customer = s.customer
             WHERE c.issuer = ? AND c.subject = ? AND s.status IN ('active', 'trialing')
             ORDER BY s.id LIMIT 1",
            &(&self.user.issuer, &self.user.subject),
        )?;
        Ok(plans.into_iter().next().and_then(|(plan,)| plan))
    }
}
