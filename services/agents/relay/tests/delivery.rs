use crate::common::Relay;
use anyhow::Result;
use serde_json::{Value, json};

mod common;

const HOST: &str = "api2.cursor.sh";
const ID: &str = "secret_11111111111111111111111111111111";
const SECOND_ID: &str = "secret_22222222222222222222222222222222";
const URL: &str = "https://api2.cursor.sh/automations/webhook/research";
const SECOND_URL: &str = "https://api2.cursor.sh/automations/webhook/second";

fn platform(url: &str, id: &str) -> Value {
    json!({"grok":{"webhook_url":url,"secret_id":id}})
}

impl Relay {
    async fn grok(&self, account: &str, id: &str, url: &str) -> Result<Value> {
        self.call_as(
            account,
            None,
            "register",
            json!({"handle":"research","platform":platform(url,id)}),
        )
        .await
    }

    async fn tick(&self) -> Result<u64> {
        Ok(self
            .call_json("relay", "deliver", json!({}))
            .await?
            .as_u64()
            .unwrap())
    }

    async fn send(&self, account: &str, key: &str) -> Result<Value> {
        self.call_as(account, None, "message", json!({
            "from":"assistant","to":"research","body":format!("Private message {key}"),"key":key,
        })).await
    }

    async fn delivery(&self) -> Result<Value> {
        self.call("delivery", json!({"agent":"research"})).await
    }
}

#[tokio::test]
async fn grok_setup_requires_the_cursor_path_and_the_calling_accounts_grant() -> Result<()> {
    let host = Relay::open().await?;
    assert!(host.grok("alice", ID, URL).await.is_err());
    host.grant_resource("other", ID, HOST, "alice", "key");
    assert!(host.grok("alice", ID, URL).await.is_err());
    host.grant_resource("relay", ID, HOST, "alice", "key");
    for url in [
        "http://api2.cursor.sh/automations/webhook/research",
        "https://api2.cursor.sh/automations/research",
        "https://api2.cursor.sh/automations/webhook/../other",
        "https://api2.cursor.sh:9443/automations/webhook/research",
        "https://api2.cursor.sh.example.org/automations/webhook/research",
        "https://api2.cursor.sh/automations/webhook/research#fragment",
    ] {
        assert!(host.grok("alice", ID, url).await.is_err(), "{url}");
    }
    let denied = host.grok("bob", ID, URL).await.unwrap_err();
    assert!(denied.to_string().contains("another account"), "{denied}");
    assert_eq!(host.grok("alice", ID, URL).await?["platform"], "grok");
    Ok(())
}

#[tokio::test]
async fn wakes_coalesce_and_keep_message_bodies_in_each_account() -> Result<()> {
    let mut host = Relay::open().await?;
    for (owner, id, url, value) in [
        ("alice", ID, URL, "alice-key"),
        ("bob", SECOND_ID, SECOND_URL, "bob-key"),
    ] {
        host.grant_resource("relay", id, HOST, owner, value);
        host.grok(owner, id, url).await?;
        host.register(owner, "Assistant").await?;
        host.arrange("POST", url, 200, "");
    }
    let one = host.send("alice", "one").await?;
    let two = host.send("alice", "two").await?;
    let three = host.send("bob", "three").await?;
    host.reopen().await?;

    let (scheduled, concurrent) = tokio::join!(host.fire(), host.tick());
    scheduled?;
    concurrent?;
    let fetched = host.fetched();
    assert_eq!(fetched.len(), 2);
    for request in fetched {
        let key = if request.url == URL {
            "alice-key"
        } else {
            "bob-key"
        };
        assert!(
            request
                .headers
                .contains(&("Authorization".into(), format!("Bearer {key}")))
        );
        assert_eq!(
            serde_json::from_slice::<Value>(&request.body)?,
            json!({"message":"@research has new messages"}),
        );
    }
    let inbox = json!({"agent":"research"});
    assert_eq!(
        host.call("inbox", inbox.clone()).await?["items"],
        json!([one, two])
    );
    assert_eq!(
        host.call_as("bob", None, "inbox", inbox).await?["items"],
        json!([three])
    );
    assert!(host.delivery().await?.is_null());
    assert_eq!(host.tick().await?, 0);
    Ok(())
}

#[tokio::test]
async fn pause_disable_and_reconnect_reconcile_pending_wakes() -> Result<()> {
    let host = Relay::open().await?;
    host.grant_resource("relay", ID, HOST, "alice", "old-key");
    host.grant_resource("relay", SECOND_ID, HOST, "alice", "new-key");
    host.grok("alice", ID, URL).await?;
    host.register("alice", "Assistant").await?;
    let message = host.send("alice", "one").await?;
    let conversation = message["conversation"].clone();
    host.call("pause", json!({"conversation":conversation,"paused":true}))
        .await?;
    assert_eq!(host.tick().await?, 0);
    host.call("pause", json!({"conversation":conversation,"paused":false}))
        .await?;
    host.call("update_agent", json!({"handle":"research","enabled":false}))
        .await?;
    assert_eq!(host.tick().await?, 0);
    host.call("update_agent", json!({"handle":"research","enabled":true}))
        .await?;
    host.call(
        "update_agent",
        json!({"handle":"research","platform":platform(SECOND_URL, SECOND_ID)}),
    )
    .await?;
    host.arrange("POST", SECOND_URL, 200, "");
    assert_eq!(host.tick().await?, 1);
    assert_eq!(host.fetched()[0].url, SECOND_URL);
    assert!(
        host.fetched()[0]
            .headers
            .contains(&("Authorization".into(), "Bearer new-key".into()))
    );

    host.send("alice", "two").await?;
    host.call(
        "update_agent",
        json!({"handle":"research","platform":"inbox"}),
    )
    .await?;
    assert_eq!(host.tick().await?, 0);
    assert!(host.delivery().await?.is_null());
    Ok(())
}

#[tokio::test]
async fn connection_backoff_survives_restart_and_exhaustion_leaves_the_inbox_intact() -> Result<()>
{
    let mut host = Relay::open().await?;
    host.grant_resource("relay", ID, HOST, "alice", "key");
    host.grok("alice", ID, URL).await?;
    host.register("alice", "Assistant").await?;
    let first = host.send("alice", "first").await?;
    let mut messages = vec![first];
    for index in 0..6 {
        host.arrange("POST", URL, 503, "");
        assert_eq!(host.tick().await?, 1);
        let state = host.delivery().await?;
        assert_eq!(state["attempts"], index + 1);
        if index < 5 {
            let next = state["next_attempt"].as_u64().unwrap();
            assert_eq!(next - host.now(), 60_000 << index);
            messages.push(host.send("alice", &format!("during-{index}")).await?);
            assert_eq!(host.delivery().await?["next_attempt"], next);
            assert_eq!(host.tick().await?, 0);
            if index == 0 {
                host.reopen().await?;
            }
            host.set_now(next);
        } else {
            assert_eq!(state["failing"], true);
            assert_eq!(state["reason"], "attempts_exhausted");
        }
    }
    messages.push(host.send("alice", "after-failure").await?);
    assert_eq!(host.tick().await?, 0);
    assert_eq!(
        host.call("inbox", json!({"agent":"research"})).await?["items"],
        json!(messages)
    );

    host.call(
        "update_agent",
        json!({"handle":"research","platform":platform(URL, ID)}),
    )
    .await?;
    host.arrange("POST", URL, 200, "");
    assert_eq!(host.tick().await?, 1);
    assert!(host.delivery().await?.is_null());
    Ok(())
}

#[tokio::test]
async fn revocation_stops_dispatch_and_acknowledging_the_inbox_clears_the_wake() -> Result<()> {
    let host = Relay::open().await?;
    host.grant_resource("relay", ID, HOST, "alice", "key");
    host.grok("alice", ID, URL).await?;
    host.register("alice", "Assistant").await?;
    let first = host.send("alice", "first").await?;
    host.revoke_resource("relay", ID, HOST);
    assert_eq!(host.tick().await?, 1);
    assert!(host.fetched().is_empty());
    assert_eq!(host.delivery().await?["reason"], "access_denied");
    assert_eq!(host.delivery().await?["failing"], true);
    host.grant_resource("relay", ID, HOST, "alice", "rotated");
    host.call(
        "update_agent",
        json!({"handle":"research","platform":platform(URL, ID)}),
    )
    .await?;
    host.call("inbox", json!({"agent":"research","after":first["id"]}))
        .await?;
    assert_eq!(host.tick().await?, 0);
    assert!(host.fetched().is_empty());
    Ok(())
}
