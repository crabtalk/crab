//! Stripe's API natively: the requests the kit sends and how it reads what
//! Stripe answers.

use berm_lang::{User, test};
use crabkit_stripe::{Customer, Price, Subscription};

const PRICES: &str = "https://api.stripe.com/v1/prices?lookup_keys%5B%5D=pro&active=true";
const CUSTOMERS: &str = "https://api.stripe.com/v1/customers";
const CHECKOUT: &str = "https://api.stripe.com/v1/checkout/sessions";
const PORTAL: &str = "https://api.stripe.com/v1/billing_portal/sessions";

fn body(fetched: &test::Fetched) -> String {
    String::from_utf8(fetched.body.clone()).expect("a form")
}

fn customer() -> Customer {
    Customer { id: "cus_1".into() }
}

#[test]
fn a_price_is_found_by_its_lookup_key() {
    test::respond(
        "GET",
        PRICES,
        200,
        br#"{"object":"list","data":[{"id":"price_1","lookup_key":"pro"}]}"#,
    );
    let price = Price::find("pro").expect("a price");
    assert_eq!(price.id, "price_1");
    assert_eq!(price.lookup_key.as_deref(), Some("pro"));

    let fetched = &test::fetched()[0];
    assert_eq!(fetched.method, "GET");
    assert!(fetched.body.is_empty());
    assert_eq!(
        fetched.header("Authorization"),
        Some("Bearer {secret:STRIPE_SECRET_KEY}")
    );
    assert_eq!(fetched.header("Stripe-Version"), Some("2025-03-31.basil"));
}

#[test]
fn no_price_with_the_lookup_key_is_refused() {
    test::respond("GET", PRICES, 200, br#"{"object":"list","data":[]}"#);
    let failure = Price::find("pro").err().expect("a refusal");
    assert_eq!(failure.message, "no active price has that lookup key");
}

#[test]
fn a_customer_carries_its_user_in_its_metadata() {
    test::respond(
        "POST",
        CUSTOMERS,
        200,
        br#"{"id":"cus_1","object":"customer"}"#,
    );
    let user = User {
        issuer: "https://auth.example.com".into(),
        subject: "7".into(),
    };
    let customer = Customer::create(&user).expect("a customer");
    assert_eq!(customer.id, "cus_1");
    assert_eq!(
        body(&test::fetched()[0]),
        "metadata%5Bissuer%5D=https%3A%2F%2Fauth.example.com&metadata%5Bsubject%5D=7"
    );
}

#[test]
fn a_checkout_subscribes_the_customer_to_the_price() {
    test::respond(
        "POST",
        CHECKOUT,
        200,
        br#"{"id":"cs_1","url":"https://checkout.stripe.com/c/cs_1"}"#,
    );
    let price = Price {
        id: "price_1".into(),
        lookup_key: Some("pro".into()),
    };
    let session = customer()
        .checkout(
            &price,
            "https://app.example.com/ok",
            "https://app.example.com/no",
        )
        .expect("a session");
    assert_eq!(session.url, "https://checkout.stripe.com/c/cs_1");
    assert_eq!(
        body(&test::fetched()[0]),
        "mode=subscription&customer=cus_1&line_items%5B0%5D%5Bprice%5D=price_1&line_items%5B0%5D%5Bquantity%5D=1&success_url=https%3A%2F%2Fapp.example.com%2Fok&cancel_url=https%3A%2F%2Fapp.example.com%2Fno"
    );
}

#[test]
fn a_portal_returns_the_customer_back() {
    test::respond(
        "POST",
        PORTAL,
        200,
        br#"{"id":"bps_1","url":"https://billing.stripe.com/p/session/1"}"#,
    );
    let session = customer()
        .portal("https://app.example.com/account")
        .expect("a session");
    assert_eq!(session.url, "https://billing.stripe.com/p/session/1");
    assert_eq!(
        body(&test::fetched()[0]),
        "customer=cus_1&return_url=https%3A%2F%2Fapp.example.com%2Faccount"
    );
}

#[test]
fn a_subscription_reads_its_plan_from_its_first_price() {
    test::respond(
        "GET",
        "https://api.stripe.com/v1/subscriptions/sub_1",
        200,
        br#"{"id":"sub_1","customer":"cus_1","status":"active","items":{"object":"list","data":[{"price":{"id":"price_1","lookup_key":"pro"}}]}}"#,
    );
    let subscription = Subscription::retrieve("sub_1").expect("a subscription");
    assert_eq!(subscription.customer, "cus_1");
    assert_eq!(subscription.status, "active");
    assert_eq!(subscription.plan(), Some("pro"));
}

#[test]
fn a_refusal_reads_as_stripe_said() {
    test::respond(
        "POST",
        PORTAL,
        400,
        br#"{"error":{"type":"invalid_request_error","message":"No such customer: 'cus_1'"}}"#,
    );
    let failure = customer()
        .portal("https://app.example.com")
        .err()
        .expect("a refusal");
    assert_eq!(
        failure.message,
        "Stripe answered 400: No such customer: 'cus_1'"
    );
}

#[test]
fn an_unreadable_answer_fails() {
    test::respond("POST", PORTAL, 200, b"<html>");
    let failure = customer()
        .portal("https://app.example.com")
        .err()
        .expect("a failure");
    assert_eq!(
        failure.message,
        "Stripe answered what this service cannot read"
    );
}
