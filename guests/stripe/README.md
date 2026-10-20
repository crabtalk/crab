# Stripe

Stripe subscriptions for a product's users: Checkout, the customer portal, and each user's plan.

A product deploys this service under its own account, with its own Stripe keys, and its other services reach it with `berm_lang::call("stripe", …)`. Each call names the end user the product's call carries; a call with none is refused.

## Tools

- `checkout(price, success, cancel)`: a Checkout session subscribing the user to the price whose `lookup_key` is `price`, sending them to `success` once they subscribe and to `cancel` if they leave. Answers `{url}` to send them to.
- `portal(back)`: a customer portal session for the user, sending them back to `back`. Answers `{url}`.
- `plan()`: the `lookup_key` of the price the user's active or trialing subscription is on, or `null`.

A plan is the `lookup_key` of one of the product's Stripe prices.

## Setting it up

1. Create a restricted key with Write on Checkout Sessions, customer portal sessions and Customers, Read on Subscriptions and Prices, and None on everything else. Set it as the secret `STRIPE_SECRET_KEY`.
2. Add a webhook endpoint at `https://<cloud>/services/<owner>/stripe/stripe` subscribed to `checkout.session.completed`, `customer.subscription.created`, `customer.subscription.updated` and `customer.subscription.deleted`. Set its signing secret as the secret `STRIPE_WEBHOOK_SECRET`.
3. Give each price a plan a user can choose a `lookup_key`.
