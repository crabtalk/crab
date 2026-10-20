# Stripe

Stripe subscriptions for a product's users: Checkout, the customer portal, and each user's plan, kept from Stripe's events.

## As a library

Add the kit to your program's crate:

```toml
[dependencies]
crab-stripe = "0.1"
```

Name it in `uses`, and call it from your own tools:

```rust,ignore
#[berm_lang::program(uses = [crab_stripe])]
mod tools {
    use alloc::string::String;
    use berm_lang::Result;
    use crab_stripe::{Data, Session, Subscriber};

    /// Opens a Checkout session subscribing the calling user to `plan`.
    #[public]
    pub fn upgrade(plan: String, success: String, cancel: String) -> Result<Session> {
        Subscriber::calling()?.checkout(&plan, &success, &cancel)
    }

    /// Keeps the subscription a Stripe event names.
    #[stripe]
    pub fn webhook(data: Data) -> Result<()> {
        data.keep()
    }
}
```

The program declares nothing the kit already declares: `STRIPE_SECRET_KEY`, `api.stripe.com` under `berm.http.fetch`, and its migrations reach the program's manifest through `uses`. Its tables are `stripe_customers` and `stripe_subscriptions`, in the program's database.

- `Subscriber::calling()`: the end user the call carries; a call with none is refused. `Subscriber { user }` names another.
- `subscriber.checkout(plan, success, cancel)`: a Checkout session subscribing the user to the price whose `lookup_key` is `plan`, sending them to `success` once they subscribe and to `cancel` if they leave. Their first checkout makes their Stripe customer. Answers `Session {url}`.
- `subscriber.portal(back)`: a customer portal session for the user, sending them back to `back`. Refused before their first checkout.
- `subscriber.plan()`: the `lookup_key` of the price the user's active or trialing subscription is on.
- `subscriber.customer()`: the user's Stripe `Customer`, once they have checked out.
- `data.keep()`: keeps the subscription a Stripe event names, a subscription's own or a Checkout session's, as Stripe answers it now. A program has one `#[stripe]` tool, which calls it.
- `Price::find`, `Customer::create`, `customer.checkout`, `customer.portal` and `Subscription::retrieve` are the Stripe API calls these make.

## As a service

`bin/` is the kit as a service of its own, `stripe`: deploy it under your own account, with your own Stripe keys, and your other services reach it with `berm_lang::call("stripe", …)`. Each call names the end user the product's call carries.

- `checkout(price, success, cancel)`: a Checkout session subscribing the user to the price whose `lookup_key` is `price`. Answers `{url}` to send them to.
- `portal(back)`: a customer portal session for the user. Answers `{url}`.
- `plan()`: the user's plan, or `null`.

## Setting it up

1. Create a restricted key with Write on Checkout Sessions, customer portal sessions and Customers, Read on Subscriptions and Prices, and None on everything else. Set it as the secret `STRIPE_SECRET_KEY`.
2. Add a webhook endpoint at `https://<cloud>/services/<owner>/<service>/stripe` subscribed to `checkout.session.completed`, `customer.subscription.created`, `customer.subscription.updated` and `customer.subscription.deleted`. Set its signing secret as the secret `STRIPE_WEBHOOK_SECRET`.
3. Give each price a plan a user can choose a `lookup_key`.
