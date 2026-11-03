use anyhow::Result;
use berm_api::Output;
use berm_build::fixture;
use berm_host::test::TestHost;
use serde_json::json;

#[tokio::test]
#[ignore = "requires a prepared Wasm image; see LIBRARY.md"]
async fn duplicate_delivery_acknowledgment_and_restart_keep_one_event() -> Result<()> {
    let mut host = TestHost::open(10_000).await?;
    host.deploy("inbox", &fixture::image("crab-webhook-inbox")?)
        .await?;
    let envelope = json!({"source":"builds","id":"build-1","payload":{"state":"passed"}});
    let receipt = host.call_json("inbox", "receive", envelope.clone()).await?;
    assert_eq!(receipt["duplicate"], false);
    assert_eq!(
        host.call_json("inbox", "receive", envelope).await?["duplicate"],
        true
    );
    let conflict = host
        .call(
            "inbox",
            "receive",
            br#"{"source":"builds","id":"build-1","payload":{"state":"failed"}}"#,
        )
        .await?;
    assert!(matches!(conflict.result, Output::Failed(_)));
    let page = host
        .call_json("inbox", "events", json!({"limit":1}))
        .await?;
    assert_eq!(page["items"].as_array().unwrap().len(), 1);
    assert!(page["items"][0].get("payload").is_none());
    assert_eq!(
        host.call_json("inbox", "get", json!({"seq":receipt["seq"]}))
            .await?["payload"]["state"],
        "passed"
    );
    assert_eq!(
        host.call_json("inbox", "acknowledge", json!({"seq":receipt["seq"]}))
            .await?,
        true
    );
    assert_eq!(
        host.call_json("inbox", "acknowledge", json!({"seq":receipt["seq"]}))
            .await?,
        false
    );
    host.reopen().await?;
    assert!(
        host.call_json("inbox", "events", json!({})).await?["items"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        host.call_json("inbox", "events", json!({"pending_only":false}))
            .await?["items"][0]["acknowledged"],
        true
    );
    Ok(())
}

#[tokio::test]
#[ignore = "requires a prepared Wasm image; see LIBRARY.md"]
async fn cursor_retention_and_payload_limit_are_explicit() -> Result<()> {
    let host = TestHost::open(10_000).await?;
    host.deploy("inbox", &fixture::image("crab-webhook-inbox")?)
        .await?;
    for id in ["a", "b"] {
        host.call_json(
            "inbox",
            "receive",
            json!({"source":"builds","id":id,"payload":{}}),
        )
        .await?;
    }
    let first = host
        .call_json("inbox", "events", json!({"limit":1}))
        .await?;
    assert_eq!(first["has_more"], true);
    let next = host
        .call_json("inbox", "events", json!({"after":first["next"],"limit":1}))
        .await?;
    assert_eq!(next["items"][0]["id"], "b");
    let refused = host
        .call(
            "inbox",
            "receive",
            &serde_json::to_vec(
                &json!({"source":"builds","id":"large","payload":"a".repeat(16384)}),
            )?,
        )
        .await?;
    assert!(matches!(refused.result, Output::Failed(_)));
    host.advance(31 * 86_400_000);
    host.call_json("inbox", "prune", json!({})).await?;
    assert!(
        host.call_json("inbox", "events", json!({"pending_only":false}))
            .await?["items"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        host.call_json(
            "inbox",
            "receive",
            json!({"source":"builds","id":"a","payload":{}})
        )
        .await?["duplicate"],
        false
    );
    Ok(())
}
