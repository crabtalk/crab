//! The keeper's `#[tool(webhook)]` tool, as its daemon hands it a verified
//! webhook.

use anyhow::Result;
use berm_host::test::{self, TestHost};
use serde_json::json;

async fn deployed() -> Result<TestHost> {
    let host = TestHost::open(0).await?;
    host.deploy("keeper", &test::build("berm-keeper")?).await?;
    Ok(host)
}

#[tokio::test]
async fn a_webhook_runs_its_tool_with_the_body_as_arguments() -> Result<()> {
    let host = deployed().await?;
    host.host
        .webhook("keeper", "hook", br#"{"word":"crab"}"#.to_vec())
        .await
        .expect("the webhook runs");
    assert_eq!(
        host.call_json("keeper", "count", json!({})).await?,
        json!(1)
    );
    Ok(())
}

#[tokio::test]
async fn a_webhook_for_a_tool_that_takes_none_is_refused() -> Result<()> {
    let host = deployed().await?;
    for tool in ["put", "missing"] {
        let denied = host
            .host
            .webhook("keeper", tool, br#"{"word":"crab"}"#.to_vec())
            .await
            .expect_err("no webhook tool");
        assert_eq!(denied.status, 404);
        assert_eq!(denied.explain, Some("E0014"));
    }
    assert_eq!(
        host.call_json("keeper", "count", json!({})).await?,
        json!(0)
    );
    Ok(())
}

#[tokio::test]
async fn a_tool_that_stops_taking_webhooks_breaks_the_service() -> Result<()> {
    let host = deployed().await?;
    let manifest = &host.host.get("keeper").expect("deployed").manifest;
    let mut next = manifest.clone();
    next.tools.iter_mut().for_each(|tool| tool.webhook = false);
    let breaks = manifest.breaks(&next);
    assert_eq!(breaks.len(), 1);
    assert_eq!(breaks[0].reason, "no longer takes webhooks");
    Ok(())
}

#[tokio::test]
async fn a_stopped_service_refuses_a_webhook() -> Result<()> {
    let host = deployed().await?;
    host.host.stop("keeper").await.expect("stopped");
    let denied = host
        .host
        .webhook("keeper", "hook", br#"{"word":"crab"}"#.to_vec())
        .await
        .expect_err("stopped");
    assert_eq!(denied.status, 503);
    assert_eq!(denied.explain, Some("E0018"));
    Ok(())
}
