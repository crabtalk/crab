//! The `stripe` service against arranged Stripe answers: customers recorded on
//! checkout, and plans kept from Stripe's events.

use anyhow::Result;
use berm_api::Output;
use berm_host::test::{self, TestHost, User};
use serde_json::{Value, json};

const PRICES: &str = "https://api.stripe.com/v1/prices?lookup_keys%5B%5D=pro&active=true";
const CUSTOMERS: &str = "https://api.stripe.com/v1/customers";
const CHECKOUT: &str = "https://api.stripe.com/v1/checkout/sessions";
const PORTAL: &str = "https://api.stripe.com/v1/billing_portal/sessions";
const SUBSCRIPTION: &str = "https://api.stripe.com/v1/subscriptions/sub_1";

async fn deployed() -> Result<TestHost> {
    let host = TestHost::open(0).await?;
    host.deploy("billing", &test::build("berm-billing")?)
        .await?;
    host.grant("billing", "STRIPE_SECRET_KEY", "api.stripe.com", "rk_test");
    host.arrange(
        "GET",
        PRICES,
        200,
        r#"{"data":[{"id":"price_1","lookup_key":"pro"}]}"#,
    );
    host.arrange("POST", CUSTOMERS, 200, r#"{"id":"cus_1"}"#);
    host.arrange(
        "POST",
        CHECKOUT,
        200,
        r#"{"url":"https://checkout.stripe.com/c/cs_1"}"#,
    );
    host.arrange(
        "POST",
        PORTAL,
        200,
        r#"{"url":"https://billing.stripe.com/p/session/1"}"#,
    );
    Ok(host)
}

fn ada() -> User {
    User {
        issuer: "https://auth.example.com".into(),
        subject: "7".into(),
    }
}

fn subscription(status: &str) -> String {
    json!({"id": "sub_1", "customer": "cus_1", "status": status, "items": {"data": [{"price": {"id": "price_1", "lookup_key": "pro"}}]}})
        .to_string()
}

/// How `tool` ended for `user`, its answer read as JSON or its failure.
async fn run(
    host: &TestHost,
    tool: &str,
    args: Value,
    user: Option<User>,
) -> Result<Result<Value, String>> {
    let args = args.to_string();
    let ran = match user {
        Some(user) => {
            host.call_for("billing", tool, args.as_bytes(), user)
                .await?
        }
        None => host.call("billing", tool, args.as_bytes()).await?,
    };
    Ok(match ran.result {
        Output::Done(answer) if answer.is_empty() => Ok(Value::Null),
        Output::Done(answer) => Ok(serde_json::from_str(&answer)?),
        Output::Failed(failure) => Err(failure),
    })
}

fn checkout() -> Value {
    json!({"price": "pro", "success": "https://app.example.com/ok", "cancel": "https://app.example.com/no"})
}

#[tokio::test]
async fn a_user_is_made_a_customer_on_their_first_checkout_only() -> Result<()> {
    let host = deployed().await?;
    for _ in 0..2 {
        let session = run(&host, "checkout", checkout(), Some(ada())).await?;
        assert_eq!(
            session,
            Ok(json!({"url": "https://checkout.stripe.com/c/cs_1"}))
        );
    }
    let made = host
        .fetched()
        .iter()
        .filter(|request| request.url == CUSTOMERS)
        .count();
    assert_eq!(made, 1);
    let rows = host
        .sql(
            "billing",
            "SELECT customer FROM stripe_customers",
            json!([]),
        )
        .await?;
    assert_eq!(rows["rows"], json!([["cus_1"]]));
    Ok(())
}

#[tokio::test]
async fn a_call_without_a_user_is_refused() -> Result<()> {
    let host = deployed().await?;
    let failure = run(&host, "plan", json!({}), None).await?;
    assert_eq!(failure, Err("this call carries no user".into()));
    Ok(())
}

#[tokio::test]
async fn the_portal_needs_a_checkout_first() -> Result<()> {
    let host = deployed().await?;
    let back = json!({"back": "https://app.example.com/account"});
    let failure = run(&host, "portal", back.clone(), Some(ada())).await?;
    assert_eq!(failure, Err("the user has never checked out".into()));

    run(&host, "checkout", checkout(), Some(ada())).await?.ok();
    let session = run(&host, "portal", back, Some(ada())).await?;
    assert_eq!(
        session,
        Ok(json!({"url": "https://billing.stripe.com/p/session/1"}))
    );
    Ok(())
}

#[tokio::test]
async fn a_plan_follows_the_subscription_stripe_answers() -> Result<()> {
    let host = deployed().await?;
    run(&host, "checkout", checkout(), Some(ada())).await?.ok();
    assert_eq!(
        run(&host, "plan", json!({}), Some(ada())).await?,
        Ok(Value::Null)
    );

    host.arrange("GET", SUBSCRIPTION, 200, subscription("active"));
    let completed = json!({"data": {"object": {"id": "cs_1", "object": "checkout.session", "subscription": "sub_1"}}});
    assert_eq!(
        run(&host, "webhook", completed, None).await?,
        Ok(Value::Null)
    );
    assert_eq!(
        run(&host, "plan", json!({}), Some(ada())).await?,
        Ok(json!("pro"))
    );

    host.arrange("GET", SUBSCRIPTION, 200, subscription("canceled"));
    let deleted = json!({"data": {"object": {"id": "sub_1", "object": "subscription"}}});
    assert_eq!(run(&host, "webhook", deleted, None).await?, Ok(Value::Null));
    assert_eq!(
        run(&host, "plan", json!({}), Some(ada())).await?,
        Ok(Value::Null)
    );
    Ok(())
}

#[tokio::test]
async fn an_event_naming_no_subscription_keeps_nothing() -> Result<()> {
    let host = deployed().await?;
    let payment = json!({"data": {"object": {"id": "cs_2", "object": "checkout.session"}}});
    assert_eq!(run(&host, "webhook", payment, None).await?, Ok(Value::Null));
    assert!(host.fetched().is_empty());
    Ok(())
}
