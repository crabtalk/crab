use anyhow::Result;
use berm_api::Output;
use berm_build::fixture;
use berm_host::test::TestHost;
use serde_json::json;

#[tokio::test]
#[ignore = "requires a prepared Wasm image; see LIBRARY.md"]
async fn consecutive_failures_recovery_staleness_and_history_survive_restart() -> Result<()> {
    let mut host = TestHost::open(10_000).await?;
    host.deploy("uptime", &fixture::image("crab-uptime")?)
        .await?;
    host.call_json(
        "uptime",
        "watch",
        json!({"name":"API","url":"https://example.com/health"}),
    )
    .await?;
    host.arrange("GET", "https://example.com/health", 503, "unavailable");
    assert_eq!(
        host.call_json("uptime", "check", json!({"name":"API"}))
            .await?["state"],
        "unknown"
    );
    assert_eq!(
        host.call_json("uptime", "check", json!({"name":"API"}))
            .await?["state"],
        "down"
    );
    host.arrange("GET", "https://example.com/health", 204, "");
    assert_eq!(
        host.call_json("uptime", "check", json!({"name":"API"}))
            .await?["state"],
        "up"
    );
    host.call_json(
        "uptime",
        "watch",
        json!({"name":"API","url":"https://example.com/health"}),
    )
    .await?;
    let history = host
        .call_json("uptime", "history", json!({"name":"API","limit":2}))
        .await?;
    assert_eq!(history["has_more"], true);
    assert_eq!(history["items"][0]["ok"], false);
    let tail = host
        .call_json(
            "uptime",
            "history",
            json!({"name":"API","after":history["next"]}),
        )
        .await?;
    assert_eq!(tail["items"][0]["ok"], true);
    host.reopen().await?;
    host.advance(600_001);
    let status = host.call_json("uptime", "status", json!({})).await?;
    assert_eq!(status[0]["state"], "stale");
    assert!(status[0].get("url").is_none());
    assert!(status[0].get("error").is_none());
    host.call_json(
        "uptime",
        "watch",
        json!({"name":"API","url":"https://example.com/other","expected_status":302}),
    )
    .await?;
    host.arrange("GET", "https://example.com/other", 302, "redirect");
    assert_eq!(
        host.call_json("uptime", "check", json!({"name":"API"}))
            .await?["state"],
        "up"
    );
    assert_eq!(
        host.call_json("uptime", "history", json!({"name":"API"}))
            .await?["items"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        host.fetched().last().unwrap().url,
        "https://example.com/other"
    );
    Ok(())
}

#[tokio::test]
#[ignore = "requires a prepared Wasm image; see LIBRARY.md"]
async fn scheduled_work_is_bounded_and_invalid_targets_never_fetch() -> Result<()> {
    let host = TestHost::open(10_000).await?;
    host.deploy("uptime", &fixture::image("crab-uptime")?)
        .await?;
    for (url, interval) in [
        ("file:///etc/passwd", 5),
        ("https://user:pass@example.com", 5),
        ("https://example.com", 1),
    ] {
        assert!(matches!(
            host.call(
                "uptime",
                "watch",
                &serde_json::to_vec(&json!({"name":"bad","url":url,"interval_minutes":interval}))?
            )
            .await?
            .result,
            Output::Failed(_)
        ));
    }
    assert!(host.fetched().is_empty());
    for index in 0..7 {
        let url = format!("https://example.com/{index}");
        host.arrange("GET", &url, 200, "ok");
        host.call_json(
            "uptime",
            "watch",
            json!({"name":index.to_string(),"url":url}),
        )
        .await?;
    }
    assert_eq!(host.call_json("uptime", "tick", json!({})).await?, 5);
    assert_eq!(host.call_json("uptime", "tick", json!({})).await?, 2);
    assert_eq!(host.call_json("uptime", "tick", json!({})).await?, 0);
    Ok(())
}
