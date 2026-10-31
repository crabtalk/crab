use anyhow::Result;
use berm_api::Output;
use berm_host::test::{self, TestHost};
use serde_json::{Value, json};
const GH: &str = "https://api.github.com/repos/example/project/releases?per_page=30&page=1";
const OSV: &str = "https://api.osv.dev/v1/query";
fn watch() -> Value {
    json!({"name":"example","package":{"ecosystem":"crates.io","name":"example","version":"1.0.0"},"owner":"example","repo":"project"})
}
fn release(body: &str) -> Value {
    json!({"id":1,"tag_name":"v2.0.0","name":"Version 2","body":body,"html_url":"https://github.com/example/project/releases/tag/v2.0.0","draft":false,"prerelease":false,"published_at":"2026-10-01T00:00:00Z"})
}
fn arrange(host: &TestHost, body: &str) {
    host.arrange(
        "GET",
        GH,
        200,
        serde_json::to_vec(&json!([release(body)])).unwrap(),
    );
    host.arrange("POST",OSV,200,br#"{"vulns":[{"id":"GHSA-1","modified":"2026-10-01T00:00:00Z","summary":"Example advisory"}]}"#);
}
async fn deployed() -> Result<TestHost> {
    let host = TestHost::open(10_000).await?;
    let service = host
        .deploy("watch", &test::build("crab-release-watch")?)
        .await?;
    assert_eq!(service.manifest.tools.len(), 6);
    arrange(&host, "Release notes");
    host.call_json("watch", "follow", json!({"watch":watch()}))
        .await?;
    Ok(host)
}
async fn history(host: &TestHost, after: u64, limit: u64) -> Result<Value> {
    host.call_json("watch", "changes", json!({"after":after,"limit":limit}))
        .await
}

#[tokio::test]
async fn baseline_changes_pagination_and_redeployment_keep_history() -> Result<()> {
    let host = deployed().await?;
    let baseline = host
        .call_json("watch", "refresh", json!({"name":"example"}))
        .await?;
    assert_eq!(baseline["snapshot"]["advisories"][0]["id"], "GHSA-1");
    assert_eq!(baseline["snapshot"]["releases"][0]["tag_name"], "v2.0.0");
    host.call_json("watch", "refresh", json!({"name":"example"}))
        .await?;
    assert_eq!(
        history(&host, 0, 100).await?["items"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    host.advance(1000);
    arrange(&host, "Edited release notes");
    host.call_json("watch", "refresh", json!({"name":"example"}))
        .await?;
    let first = history(&host, 0, 1).await?;
    assert_eq!(first["items"][0]["kind"], "baseline");
    assert_eq!(first["has_more"], true);
    let second = history(&host, first["next_cursor"].as_u64().unwrap(), 1).await?;
    assert_eq!(second["items"][0]["kind"], "changed");
    assert_eq!(second["has_more"], false);
    host.deploy("watch", &test::build("crab-release-watch")?)
        .await?;
    assert_eq!(
        history(&host, 0, 100).await?["items"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    host.call_json("watch", "unfollow", json!({"name":"example"}))
        .await?;
    assert_eq!(
        host.call_json("watch", "watches", json!({})).await?,
        json!([])
    );
    assert_eq!(
        history(&host, 0, 100).await?["items"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    Ok(())
}

#[tokio::test]
async fn failed_and_incomplete_checks_keep_previous_snapshot_then_record_recovery() -> Result<()> {
    let host = deployed().await?;
    host.call_json("watch", "refresh", json!({"name":"example"}))
        .await?;
    host.advance(1000);
    host.arrange("POST", OSV, 200, br#"{"next_page_token":"again"}"#);
    let failed = host
        .call_json("watch", "refresh", json!({"name":"example"}))
        .await?;
    assert!(failed["error"].as_str().unwrap().contains("incomplete"));
    assert_eq!(failed["succeeded_at"], 10_000);
    assert_eq!(failed["snapshot"]["advisories"][0]["id"], "GHSA-1");
    host.call_json("watch", "refresh", json!({"name":"example"}))
        .await?;
    assert_eq!(
        history(&host, 0, 100).await?["items"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    arrange(&host, "Release notes");
    host.advance(1000);
    let recovered = host
        .call_json("watch", "refresh", json!({"name":"example"}))
        .await?;
    assert!(recovered.get("error").is_none());
    let changes = history(&host, 0, 100).await?;
    assert_eq!(changes["items"][1]["kind"], "failed");
    assert_eq!(changes["items"][2]["kind"], "recovered");
    Ok(())
}

#[tokio::test]
async fn scheduled_checks_are_bounded_and_mapping_updates_reset_the_baseline() -> Result<()> {
    let host = deployed().await?;
    for n in 1..7 {
        let mut spec = watch();
        spec["name"] = json!(format!("example-{n}"));
        host.call_json("watch", "follow", json!({"watch":spec}))
            .await?;
    }
    assert_eq!(
        host.call_json("watch", "tick", json!({})).await?["checked"],
        5
    );
    assert_eq!(
        host.call_json("watch", "tick", json!({})).await?["checked"],
        2
    );
    assert_eq!(
        host.call_json("watch", "tick", json!({})).await?["checked"],
        0
    );
    host.advance(3_600_000);
    assert_eq!(
        host.call_json("watch", "tick", json!({})).await?["checked"],
        5
    );
    let mut spec = watch();
    spec["package"]["version"] = json!("2.0.0");
    host.call_json("watch", "follow", json!({"watch":spec}))
        .await?;
    let statuses = host.call_json("watch", "watches", json!({})).await?;
    assert!(statuses[0].get("snapshot").is_none());
    host.call_json("watch", "refresh", json!({"name":"example"}))
        .await?;
    let changes = history(&host, 0, 100).await?;
    assert_eq!(
        changes["items"].as_array().unwrap().last().unwrap()["kind"],
        "baseline"
    );
    Ok(())
}

#[tokio::test]
async fn invalid_and_unknown_watches_are_refused_without_fetches() -> Result<()> {
    let host = deployed().await?;
    let mut spec = watch();
    spec["owner"] = json!("");
    assert!(matches!(
        host.call(
            "watch",
            "follow",
            &serde_json::to_vec(&json!({"watch":spec}))?
        )
        .await?
        .result,
        Output::Failed(_)
    ));
    assert!(matches!(
        host.call("watch", "refresh", br#"{"name":"unknown"}"#)
            .await?
            .result,
        Output::Failed(_)
    ));
    assert!(host.fetched().is_empty());
    Ok(())
}
