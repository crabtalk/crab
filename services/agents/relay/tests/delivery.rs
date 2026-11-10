use crate::common::{call, call_as, opened, register};
use anyhow::Result;
use berm::syscall::http::Response;
use berm_host::test::TestHost;
use serde_json::{Value, json};
use std::time::Duration;

mod common;

const ID: &str = "secret_11111111111111111111111111111111";
const NEXT_ID: &str = "secret_22222222222222222222222222222222";
const URL: &str = "https://bots.example/routine";

async fn grok(host: &TestHost, owner: &str, id: &str, url: &str) -> Result<u64> {
    Ok(call(
        host,
        owner,
        "register",
        json!({"name":"Research", "platform":{"grok":{"webhook_url":url,"secret_id":id}}}),
    )
    .await?["id"]
        .as_u64()
        .unwrap())
}

async fn tick(host: &TestHost) -> Result<u64> {
    Ok(host
        .call_json("relay", "deliver", json!({}))
        .await?
        .as_u64()
        .unwrap())
}

async fn send(
    host: &TestHost,
    from: u64,
    to: u64,
    conversation: &Value,
    key: &str,
) -> Result<Value> {
    call(host, "alice", "send", json!({"conversation":conversation,"from":from,"to":to,"body":format!("Investigate {{secret:{ID}}} and keep {{secret:literal"),"key":key})).await
}

#[tokio::test]
async fn setup_requires_the_secret_owner_service_and_exact_destination_grant() -> Result<()> {
    let host = opened().await?;
    assert!(grok(&host, "alice", ID, URL).await.is_err());
    host.grant_resource("relay", ID, "bots.example", "bob", "private");
    assert!(grok(&host, "alice", ID, URL).await.is_err());
    host.grant_resource("other", NEXT_ID, "bots.example", "alice", "other-service");
    assert!(grok(&host, "alice", NEXT_ID, URL).await.is_err());
    host.grant_resource("relay", ID, "bots.example", "alice", "private");
    assert!(
        grok(&host, "alice", ID, "https://evil.example/routine")
            .await
            .is_err()
    );
    let agent = grok(&host, "alice", ID, URL).await?;
    assert!(call(&host, "alice", "update_agent", json!({"id":agent,"platform":{"grok":{"webhook_url":"https://bots.example/other","secret_id":NEXT_ID}}})).await.is_err());
    assert_eq!(
        call(&host, "alice", "agents", json!({}))
            .await?
            .as_array()
            .unwrap()
            .len(),
        1
    );
    Ok(())
}

#[tokio::test]
async fn grok_acceptance_and_dot_replies_share_stable_message_context() -> Result<()> {
    let host = opened().await?;
    host.grant_resource("relay", ID, "bots.example", "alice", "private-key");
    host.arrange("POST", URL, 200, "private-key must not be returned");
    let bot = grok(&host, "alice", ID, URL).await?;
    let dot = register(&host, "alice", "Assistant").await?;
    let conversation =
        call(&host, "alice", "open", json!({"members":[bot,dot]})).await?["id"].clone();
    let message = send(&host, dot, bot, &conversation, "request").await?;
    assert_eq!(
        send(&host, dot, bot, &conversation, "request").await?,
        message
    );
    assert!(host.fetched().is_empty());
    assert_eq!(
        call(&host, "alice", "delivery", json!({"message":message["id"]})).await?["state"],
        "pending"
    );
    assert_eq!(tick(&host).await?, 1);
    assert_eq!(tick(&host).await?, 0);
    let fetched = host.fetched();
    assert_eq!(fetched.len(), 1);
    assert!(
        fetched[0]
            .headers
            .contains(&("Authorization".into(), "Bearer private-key".into()))
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&fetched[0].body)?,
        json!({"event":"message.created","message":message})
    );
    let state = call(&host, "alice", "delivery", json!({"message":message["id"]})).await?;
    assert_eq!(state["state"], "accepted");
    assert_eq!(state["status"], 200);
    assert_eq!(state["attempts"], 1);
    assert!(!state.to_string().contains("private-key"));
    assert!(
        call(
            &host,
            "alice",
            "retry_delivery",
            json!({"message":message["id"]})
        )
        .await
        .is_err()
    );
    call(
        &host,
        "alice",
        "grant_agent",
        json!({"agent":bot,"key_id":"bot-key"}),
    )
    .await?;
    let reply = call_as(&host, "alice", Some("bot-key"), "send", json!({"conversation":conversation,"from":bot,"to":dot,"body":"Which customer?","key":"reply","reply_to":message["id"]})).await?;
    let events = call(
        &host,
        "alice",
        "message_events",
        json!({"agent":dot,"cursor":"0"}),
    )
    .await?;
    assert_eq!(events["events"][0]["data"], reply);
    assert!(
        call(&host, "alice", "delivery", json!({"message":reply["id"]}))
            .await?
            .is_null()
    );
    assert_eq!(tick(&host).await?, 0);
    assert!(
        call(&host, "bob", "delivery", json!({"message":message["id"]}))
            .await
            .is_err()
    );
    Ok(())
}

#[tokio::test]
async fn retries_survive_restart_hold_order_pause_and_disable_and_resolve_rotated_values()
-> Result<()> {
    let mut host = opened().await?;
    host.grant_resource("relay", ID, "bots.example", "alice", "first-key");
    host.fetch.respond(
        "POST",
        URL,
        Response {
            status: 429,
            headers: vec![("Retry-After".into(), "300".into())],
            body: Vec::new(),
        },
        Duration::ZERO,
    );
    let bot = grok(&host, "alice", ID, URL).await?;
    let dot = register(&host, "alice", "Assistant").await?;
    let conversation =
        call(&host, "alice", "open", json!({"members":[bot,dot]})).await?["id"].clone();
    let first = send(&host, dot, bot, &conversation, "first").await?;
    send(&host, dot, bot, &conversation, "second").await?;
    assert_eq!(tick(&host).await?, 1);
    let pending = call(&host, "alice", "delivery", json!({"message":first["id"]})).await?;
    assert_eq!(pending["next_attempt"], host.now() + 300_000);
    host.reopen().await?;
    host.advance(300_000);
    call(
        &host,
        "alice",
        "pause",
        json!({"conversation":conversation,"paused":true}),
    )
    .await?;
    assert_eq!(tick(&host).await?, 0);
    call(
        &host,
        "alice",
        "pause",
        json!({"conversation":conversation,"paused":false}),
    )
    .await?;
    call(
        &host,
        "alice",
        "update_agent",
        json!({"id":bot,"enabled":false}),
    )
    .await?;
    assert_eq!(tick(&host).await?, 0);
    call(
        &host,
        "alice",
        "update_agent",
        json!({"id":bot,"enabled":true}),
    )
    .await?;
    host.grant_resource("relay", ID, "bots.example", "alice", "rotated-key");
    host.arrange("POST", URL, 200, "");
    assert_eq!(tick(&host).await?, 1);
    assert_eq!(tick(&host).await?, 1);
    let fetched = host.fetched();
    assert_eq!(fetched.len(), 3);
    assert_eq!(fetched[0].body, fetched[1].body);
    assert!(
        fetched[1]
            .headers
            .contains(&("Authorization".into(), "Bearer rotated-key".into()))
    );
    Ok(())
}

#[tokio::test]
async fn revoked_credentials_and_connection_changes_require_an_owner_retry() -> Result<()> {
    let host = opened().await?;
    host.grant_resource("relay", ID, "bots.example", "alice", "key");
    host.grant_resource("relay", NEXT_ID, "bots.example", "alice", "next-key");
    host.arrange("POST", URL, 200, "");
    host.arrange("POST", "https://bots.example/new", 200, "");
    let bot = grok(&host, "alice", ID, URL).await?;
    let dot = register(&host, "alice", "Assistant").await?;
    let conversation =
        call(&host, "alice", "open", json!({"members":[bot,dot]})).await?["id"].clone();
    let first = send(&host, dot, bot, &conversation, "first").await?;
    host.revoke_resource("relay", ID, "bots.example");
    assert_eq!(tick(&host).await?, 1);
    assert!(host.fetched().is_empty());
    let state = call(&host, "alice", "delivery", json!({"message":first["id"]})).await?;
    assert_eq!(state["state"], "failed");
    assert_eq!(state["reason"], "access_denied");
    assert!(
        call(
            &host,
            "alice",
            "retry_delivery",
            json!({"message":first["id"]})
        )
        .await
        .is_err()
    );
    host.grant_resource("relay", ID, "bots.example", "alice", "restored-key");
    call(
        &host,
        "alice",
        "retry_delivery",
        json!({"message":first["id"]}),
    )
    .await?;
    call(&host, "alice", "update_agent", json!({"id":bot,"platform":{"grok":{"webhook_url":"https://bots.example/new","secret_id":NEXT_ID}}})).await?;
    assert_eq!(tick(&host).await?, 0);
    assert_eq!(
        call(&host, "alice", "delivery", json!({"message":first["id"]})).await?["state"],
        "cancelled"
    );
    call(
        &host,
        "alice",
        "retry_delivery",
        json!({"message":first["id"]}),
    )
    .await?;
    assert_eq!(tick(&host).await?, 1);
    assert_eq!(host.fetched()[0].url, "https://bots.example/new");
    Ok(())
}

#[tokio::test]
async fn transient_failures_stop_after_six_attempts_and_permanent_failures_do_not_retry()
-> Result<()> {
    let host = opened().await?;
    host.grant_resource("relay", ID, "bots.example", "alice", "key");
    let bot = grok(&host, "alice", ID, URL).await?;
    let dot = register(&host, "alice", "Assistant").await?;
    let conversation =
        call(&host, "alice", "open", json!({"members":[bot,dot]})).await?["id"].clone();
    let first = send(&host, dot, bot, &conversation, "first").await?;
    for attempt in 1..=6 {
        assert_eq!(tick(&host).await?, 1);
        let state = call(&host, "alice", "delivery", json!({"message":first["id"]})).await?;
        assert_eq!(state["attempts"], attempt);
        if attempt == 1 {
            assert!(state["status"].is_null());
            host.arrange("POST", URL, 503, "");
        }
        if let Some(next) = state["next_attempt"].as_u64() {
            host.set_now(next);
        }
        if attempt == 6 {
            assert_eq!(state["state"], "failed");
            assert_eq!(state["reason"], "attempts_exhausted");
        }
    }
    assert_eq!(tick(&host).await?, 0);
    for status in [201, 302, 401, 403, 404] {
        host.arrange("POST", URL, status, "private");
        let message = send(&host, dot, bot, &conversation, &format!("status-{status}")).await?;
        assert_eq!(tick(&host).await?, 1);
        assert_eq!(
            call(&host, "alice", "delivery", json!({"message":message["id"]})).await?["state"],
            "failed"
        );
        assert_eq!(tick(&host).await?, 0);
    }
    Ok(())
}

#[tokio::test]
async fn cron_survives_restart_and_serializes_concurrent_delivery_attempts() -> Result<()> {
    let mut host = opened().await?;
    host.grant_resource("relay", ID, "bots.example", "alice", "key");
    host.arrange("POST", URL, 200, "");
    let bot = grok(&host, "alice", ID, URL).await?;
    let dot = register(&host, "alice", "Assistant").await?;
    let conversation =
        call(&host, "alice", "open", json!({"members":[bot,dot]})).await?["id"].clone();
    let message = send(&host, dot, bot, &conversation, "queued").await?;
    host.reopen().await?;
    let due = host
        .wakes()
        .await?
        .into_iter()
        .find(|(wake, _)| wake.tool == "deliver")
        .unwrap()
        .1;
    host.set_now(due);
    let (scheduled, concurrent) = tokio::join!(host.fire(), tick(&host));
    scheduled?;
    concurrent?;
    assert_eq!(host.fetched().len(), 1);
    assert_eq!(
        call(&host, "alice", "delivery", json!({"message":message["id"]})).await?["state"],
        "accepted"
    );
    assert!(
        host.wakes()
            .await?
            .into_iter()
            .any(|(wake, at)| wake.tool == "deliver" && at > due)
    );
    Ok(())
}

#[tokio::test]
async fn platform_changes_cancel_queued_posts_and_stop_the_old_event_source() -> Result<()> {
    let host = opened().await?;
    host.grant_resource("relay", ID, "bots.example", "alice", "key");
    let from = register(&host, "alice", "Sender").await?;
    let to = register(&host, "alice", "Recipient").await?;
    let conversation =
        call(&host, "alice", "open", json!({"members":[from,to]})).await?["id"].clone();
    call(&host, "alice", "message_events", json!({"agent":to})).await?;
    call(
        &host,
        "alice",
        "update_agent",
        json!({"id":to,"platform":{"grok":{"webhook_url":URL,"secret_id":ID}}}),
    )
    .await?;
    assert!(
        call(
            &host,
            "alice",
            "message_events",
            json!({"agent":to,"cursor":"0"})
        )
        .await
        .is_err()
    );
    let message = send(&host, from, to, &conversation, "queued").await?;
    call(
        &host,
        "alice",
        "update_agent",
        json!({"id":to,"platform":{"open_ai_dot":{}}}),
    )
    .await?;
    assert_eq!(tick(&host).await?, 0);
    assert_eq!(
        call(&host, "alice", "delivery", json!({"message":message["id"]})).await?["state"],
        "cancelled"
    );
    assert_eq!(
        call(
            &host,
            "alice",
            "message_events",
            json!({"agent":to,"cursor":"0"})
        )
        .await?["events"][0]["data"],
        message
    );
    assert!(host.fetched().is_empty());
    Ok(())
}
