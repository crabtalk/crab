-- Each user's Stripe customer, and each customer's subscriptions as Stripe last answered them.
-- `plan` is the `lookup_key` of the subscription's price.

CREATE TABLE stripe_customers (
    issuer TEXT NOT NULL,
    subject TEXT NOT NULL,
    customer TEXT NOT NULL UNIQUE,
    PRIMARY KEY (issuer, subject)
);

CREATE TABLE stripe_subscriptions (
    id TEXT PRIMARY KEY,
    customer TEXT NOT NULL,
    status TEXT NOT NULL,
    plan TEXT
);

CREATE INDEX stripe_subscriptions_customer ON stripe_subscriptions (customer);
