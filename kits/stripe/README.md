# Stripe

Stripe subscriptions for a product's users: Checkout, the customer portal, and each user's plan, kept from Stripe's events.

## Using it

Add the kit to your program's crate:

```toml
[dependencies]
crabkit = { version = "0.1", features = ["stripe"] }
```

The kit is also the crate `crabkit-stripe` on its own.

Call it from your own tools:

```rust,ignore
use alloc::string::String;
use berm_lang::{Result, tool};
use crabkit::stripe::{Data, Session, Subscriber};

/// Opens a Checkout session subscribing the calling user to `plan`.
#[tool(public)]
pub fn upgrade(plan: String, success: String, cancel: String) -> Result<Session> {
    Subscriber::calling()?.checkout(&plan, &success, &cancel)
}

/// Keeps the subscription a Stripe event names.
#[tool(stripe)]
pub fn webhook(data: Data) -> Result<()> {
    data.keep()
}
```

The program declares nothing the kit already declares: the kit's `crab.toml` declares `STRIPE_SECRET_KEY`, `api.stripe.com` under `berm.http.fetch`, and its migrations, and they reach the program's manifest when the program links the kit. Its tables are `stripe_customers` and `stripe_subscriptions`, in the program's database.

- `Subscriber::calling()`: the end user the call carries; a call with none is refused. `Subscriber { user }` names another.
- `subscriber.checkout(plan, success, cancel)`: a Checkout session subscribing the user to the price whose `lookup_key` is `plan`, sending them to `success` once they subscribe and to `cancel` if they leave. Their first checkout makes their Stripe customer. Answers `Session {url}`.
- `subscriber.portal(back)`: a customer portal session for the user, sending them back to `back`. Refused before their first checkout.
- `subscriber.plan()`: the `lookup_key` of the price the user's active or trialing subscription is on.
- `subscriber.customer()`: the user's Stripe `Customer`, once they have checked out.
- `data.keep()`: keeps the subscription a Stripe event names, a subscription's own or a Checkout session's, as Stripe answers it now. A program has one `#[tool(stripe)]` tool, which calls it.
- `Price::find`, `Customer::create`, `customer.checkout`, `customer.portal` and `Subscription::retrieve` are the Stripe API calls these make.

## Setting it up

1. Create a restricted key with Write on Checkout Sessions, customer portal sessions and Customers, Read on Subscriptions and Prices, and None on everything else. Set it as the secret `STRIPE_SECRET_KEY`.
2. Add a webhook endpoint at `https://<cloud>/services/<owner>/<service>/stripe` subscribed to `checkout.session.completed`, `customer.subscription.created`, `customer.subscription.updated` and `customer.subscription.deleted`. Set its signing secret as the secret `STRIPE_WEBHOOK_SECRET`.
3. Give each price a plan a user can choose a `lookup_key`.
