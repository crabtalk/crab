use crate::common::Relay;
use anyhow::{Result, bail};
use berm::syscall::http::Response;
use berm_api::Output;
use serde_json::{Value, json};
use std::time::Duration;

mod common;

const ID: &str = "secret_11111111111111111111111111111111";
const NEXT_ID: &str = "secret_22222222222222222222222222222222";
const URL: &str = "https://bots.example/routine";

impl Relay {
    async fn grok(&self, owner: &str, id: &str, url: &str) -> Result<u64> {
        Ok(self
            .call_as(
                owner,
                None,
                "register",
                json!({"name":"Research", "platform":{"grok":{"webhook_url":url,"secret_id":id}}}),
            )
            .await?["id"]
            .as_u64()
            .unwrap())
    }

    async fn tick(&self) -> Result<u64> {
        let mut attempted = 0;
        for (_, ran) in self.fire().await? {
            let Output::Done(result) = ran.result else {
                bail!("delivery failed: {:?}", ran.result);
            };
            attempted += serde_json::from_str::<u64>(&result)?;
        }
        Ok(attempted)
    }

    async fn send(&self, from: u64, to: u64, conversation: u64, key: &str) -> Result<Value> {
        self.call("send", json!({"conversation":conversation,"from":from,"to":to,"body":format!("Investigate {{secret:{ID}}} and keep {{secret:literal"),"key":key})).await
    }
}

#[tokio::test]
async fn setup_requires_the_secret_owner_service_and_exact_destination_grant() -> Result<()> {
    let host = Relay::open().await?;
    assert!(host.grok("alice", ID, URL).await.is_err());
    host.grant_resource("relay", ID, "bots.example", "bob", "private");
    assert!(host.grok("alice", ID, URL).await.is_err());
    host.grant_resource("other", NEXT_ID, "bots.example", "alice", "other-service");
    assert!(host.grok("alice", NEXT_ID, URL).await.is_err());
    host.grant_resource("relay", ID, "bots.example", "alice", "private");
    assert!(
        host.grok("alice", ID, "https://evil.example/routine")
            .await
            .is_err()
    );
    let agent = host.grok("alice", ID, URL).await?;
    assert!(host.call("update_agent", json!({"id":agent,"platform":{"grok":{"webhook_url":"https://bots.example/other","secret_id":NEXT_ID}}})).await.is_err());
    assert_eq!(
        host.call("agents", json!({}))
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
    let host = Relay::open().await?;
    assert!(host.wakes().await?.is_empty());
    assert!(host.host.crons("relay").await?.crons.is_empty());
    host.grant_resource("relay", ID, "bots.example", "alice", "private-key");
    host.arrange("POST", URL, 200, "private-key must not be returned");
    let bot = host.grok("alice", ID, URL).await?;
    let dot = host.register("alice", "Assistant").await?;
    let conversation = host.conversation(&[bot, dot]).await?;
    let message = host.send(dot, bot, conversation, "request").await?;
    assert_eq!(host.send(dot, bot, conversation, "request").await?, message);
    assert!(host.fetched().is_empty());
    let wakes = host.wakes().await?;
    assert_eq!(wakes.len(), 1);
    assert_eq!(wakes[0].1, host.now());
    assert_eq!(
        host.call("delivery", json!({"message":message["id"]}))
            .await?["state"],
        "pending"
    );
    assert_eq!(host.tick().await?, 1);
    assert_eq!(host.tick().await?, 0);
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
    let state = host
        .call("delivery", json!({"message":message["id"]}))
        .await?;
    assert_eq!(state["state"], "accepted");
    assert_eq!(state["status"], 200);
    assert_eq!(state["attempts"], 1);
    assert!(!state.to_string().contains("private-key"));
    assert!(
        host.call("retry_delivery", json!({"message":message["id"]}))
            .await
            .is_err()
    );
    host.call("grant_agent", json!({"agent":bot,"key_id":"bot-key"}))
        .await?;
    let reply = host.call_as("alice", Some("bot-key"), "send", json!({"conversation":conversation,"from":bot,"to":dot,"body":"Which customer?","key":"reply","reply_to":message["id"]})).await?;
    let events = host
        .call("message_events", json!({"agent":dot,"cursor":"0"}))
        .await?;
    assert_eq!(events["events"][0]["data"], reply);
    assert!(
        host.call("delivery", json!({"message":reply["id"]}))
            .await?
            .is_null()
    );
    assert_eq!(host.tick().await?, 0);
    assert!(
        host.call_as("bob", None, "delivery", json!({"message":message["id"]}))
            .await
            .is_err()
    );
    Ok(())
}

#[tokio::test]
async fn retries_survive_restart_hold_order_pause_and_disable_and_resolve_rotated_values()
-> Result<()> {
    let mut host = Relay::open().await?;
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
    let bot = host.grok("alice", ID, URL).await?;
    let dot = host.register("alice", "Assistant").await?;
    let conversation = host.conversation(&[bot, dot]).await?;
    let first = host.send(dot, bot, conversation, "first").await?;
    host.send(dot, bot, conversation, "second").await?;
    assert_eq!(host.tick().await?, 1);
    let pending = host
        .call("delivery", json!({"message":first["id"]}))
        .await?;
    assert_eq!(pending["next_attempt"], host.now() + 300_000);
    assert_eq!(host.wakes().await?[0].1, host.now() + 300_000);
    assert_eq!(host.tick().await?, 0);
    host.reopen().await?;
    host.advance(300_000);
    host.call("pause", json!({"conversation":conversation,"paused":true}))
        .await?;
    assert_eq!(host.tick().await?, 0);
    assert!(host.wakes().await?.is_empty());
    host.call("pause", json!({"conversation":conversation,"paused":false}))
        .await?;
    host.call("update_agent", json!({"id":bot,"enabled":false}))
        .await?;
    assert_eq!(host.tick().await?, 0);
    assert!(host.wakes().await?.is_empty());
    host.call("update_agent", json!({"id":bot,"enabled":true}))
        .await?;
    host.grant_resource("relay", ID, "bots.example", "alice", "rotated-key");
    host.arrange("POST", URL, 200, "");
    assert_eq!(host.tick().await?, 1);
    assert_eq!(host.tick().await?, 1);
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
    let host = Relay::open().await?;
    host.grant_resource("relay", ID, "bots.example", "alice", "key");
    host.grant_resource("relay", NEXT_ID, "bots.example", "alice", "next-key");
    host.arrange("POST", URL, 200, "");
    host.arrange("POST", "https://bots.example/new", 200, "");
    let bot = host.grok("alice", ID, URL).await?;
    let dot = host.register("alice", "Assistant").await?;
    let conversation = host.conversation(&[bot, dot]).await?;
    let first = host.send(dot, bot, conversation, "first").await?;
    host.revoke_resource("relay", ID, "bots.example");
    assert_eq!(host.tick().await?, 1);
    assert!(host.fetched().is_empty());
    let state = host
        .call("delivery", json!({"message":first["id"]}))
        .await?;
    assert_eq!(state["state"], "failed");
    assert_eq!(state["reason"], "access_denied");
    assert!(
        host.call("retry_delivery", json!({"message":first["id"]}))
            .await
            .is_err()
    );
    host.grant_resource("relay", ID, "bots.example", "alice", "restored-key");
    host.call("retry_delivery", json!({"message":first["id"]}))
        .await?;
    host.call("update_agent", json!({"id":bot,"platform":{"grok":{"webhook_url":"https://bots.example/new","secret_id":NEXT_ID}}})).await?;
    assert_eq!(host.tick().await?, 0);
    assert_eq!(
        host.call("delivery", json!({"message":first["id"]}))
            .await?["state"],
        "cancelled"
    );
    host.call("retry_delivery", json!({"message":first["id"]}))
        .await?;
    assert_eq!(host.tick().await?, 1);
    assert_eq!(host.fetched()[0].url, "https://bots.example/new");
    Ok(())
}

#[tokio::test]
async fn transient_failures_stop_after_six_attempts_and_permanent_failures_do_not_retry()
-> Result<()> {
    let host = Relay::open().await?;
    host.grant_resource("relay", ID, "bots.example", "alice", "key");
    let bot = host.grok("alice", ID, URL).await?;
    let dot = host.register("alice", "Assistant").await?;
    let conversation = host.conversation(&[bot, dot]).await?;
    let first = host.send(dot, bot, conversation, "first").await?;
    for attempt in 1..=6 {
        assert_eq!(host.tick().await?, 1);
        let state = host
            .call("delivery", json!({"message":first["id"]}))
            .await?;
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
    assert_eq!(host.tick().await?, 0);
    for status in [201, 302, 401, 403, 404] {
        host.arrange("POST", URL, status, "private");
        let message = host
            .send(dot, bot, conversation, &format!("status-{status}"))
            .await?;
        assert_eq!(host.tick().await?, 1);
        assert_eq!(
            host.call("delivery", json!({"message":message["id"]}))
                .await?["state"],
            "failed"
        );
        assert_eq!(host.tick().await?, 0);
    }
    Ok(())
}

#[tokio::test]
async fn an_immediate_wake_survives_restart_and_serializes_concurrent_delivery_attempts()
-> Result<()> {
    let mut host = Relay::open().await?;
    host.grant_resource("relay", ID, "bots.example", "alice", "key");
    host.arrange("POST", URL, 200, "");
    let bot = host.grok("alice", ID, URL).await?;
    let dot = host.register("alice", "Assistant").await?;
    let conversation = host.conversation(&[bot, dot]).await?;
    let message = host.send(dot, bot, conversation, "queued").await?;
    host.reopen().await?;
    let due = host
        .wakes()
        .await?
        .into_iter()
        .find(|(wake, _)| wake.tool == "deliver")
        .unwrap()
        .1;
    assert_eq!(due, host.now());
    let (scheduled, concurrent) =
        tokio::join!(host.fire(), host.call_json("relay", "deliver", json!({})));
    scheduled?;
    concurrent?;
    assert_eq!(host.fetched().len(), 1);
    assert_eq!(
        host.call("delivery", json!({"message":message["id"]}))
            .await?["state"],
        "accepted"
    );
    assert!(host.wakes().await?.is_empty());
    Ok(())
}

#[tokio::test]
async fn platform_changes_cancel_queued_posts_and_stop_the_old_event_source() -> Result<()> {
    let host = Relay::open().await?;
    host.grant_resource("relay", ID, "bots.example", "alice", "key");
    let from = host.register("alice", "Sender").await?;
    let to = host.register("alice", "Recipient").await?;
    let conversation = host.conversation(&[from, to]).await?;
    host.call("message_events", json!({"agent":to})).await?;
    host.call(
        "update_agent",
        json!({"id":to,"platform":{"grok":{"webhook_url":URL,"secret_id":ID}}}),
    )
    .await?;
    assert!(
        host.call("message_events", json!({"agent":to,"cursor":"0"}))
            .await
            .is_err()
    );
    let message = host.send(from, to, conversation, "queued").await?;
    host.call(
        "update_agent",
        json!({"id":to,"platform":{"open_ai_dot":{}}}),
    )
    .await?;
    assert_eq!(host.tick().await?, 0);
    assert_eq!(
        host.call("delivery", json!({"message":message["id"]}))
            .await?["state"],
        "cancelled"
    );
    assert_eq!(
        host.call("message_events", json!({"agent":to,"cursor":"0"}))
            .await?["events"][0]["data"],
        message
    );
    assert!(host.fetched().is_empty());
    Ok(())
}

#[tokio::test]
async fn new_messages_preserve_the_earliest_retry_and_wait_for_their_recipient() -> Result<()> {
    let host = Relay::open().await?;
    host.grant_resource("relay", ID, "bots.example", "alice", "key");
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
    let other_url = "https://bots.example/other";
    host.arrange("POST", other_url, 503, "");
    let first_bot = host.grok("alice", ID, URL).await?;
    let other_bot = host.grok("alice", ID, other_url).await?;
    let dot = host.register("alice", "Assistant").await?;
    let conversation = host.conversation(&[first_bot, other_bot, dot]).await?;
    let first = host.send(dot, first_bot, conversation, "first").await?;
    assert_eq!(host.tick().await?, 1);
    let first_due = host.wakes().await?[0].1;

    host.advance(1000);
    host.send(dot, first_bot, conversation, "second").await?;
    assert_eq!(host.wakes().await?[0].1, first_due);
    assert_eq!(host.tick().await?, 0);

    host.send(dot, other_bot, conversation, "other").await?;
    assert_eq!(host.wakes().await?[0].1, host.now());
    assert_eq!(host.tick().await?, 1);
    let other_due = host.wakes().await?[0].1;
    assert!(other_due < first_due);
    host.advance(1000);
    host.send(dot, first_bot, conversation, "third").await?;
    assert_eq!(host.wakes().await?[0].1, other_due);

    host.arrange("POST", other_url, 200, "");
    host.set_now(other_due);
    assert_eq!(host.tick().await?, 1);
    assert_eq!(host.wakes().await?[0].1, first_due);
    host.arrange("POST", URL, 200, "");
    host.set_now(first_due);
    for _ in 0..3 {
        assert_eq!(host.tick().await?, 1);
    }
    let fetched = host.fetched();
    assert_eq!(fetched.len(), 6);
    let retried: Value = serde_json::from_slice(&fetched[3].body)?;
    assert_eq!(retried["message"], first);
    assert!(host.wakes().await?.is_empty());
    Ok(())
}

#[tokio::test]
async fn a_queue_larger_than_one_batch_rearms_until_empty() -> Result<()> {
    let host = Relay::open().await?;
    host.grant_resource("relay", ID, "bots.example", "alice", "key");
    host.arrange("POST", URL, 200, "");
    let dot = host.register("alice", "Assistant").await?;
    let mut members = vec![dot];
    for _ in 0..6 {
        members.push(host.grok("alice", ID, URL).await?);
    }
    let conversation = host.conversation(&members).await?;
    for bot in &members[1..] {
        host.send(dot, *bot, conversation, &format!("message-{bot}"))
            .await?;
    }
    assert_eq!(host.wakes().await?.len(), 1);
    assert_eq!(host.tick().await?, 5);
    assert_eq!(host.wakes().await?[0].1, host.now());
    assert_eq!(host.tick().await?, 1);
    assert_eq!(host.fetched().len(), 6);
    assert!(host.wakes().await?.is_empty());
    host.advance(86_400_000);
    assert!(host.fire().await?.is_empty());
    Ok(())
}
