use anyhow::Result;
use berm_api::Output;
use berm_build::fixture;
use berm_host::test::TestHost;
use serde_json::json;

#[tokio::test]
async fn page_views_crawler_reports_and_restart_preserve_counts() -> Result<()> {
    let mut host = TestHost::open(40 * 86_400_000).await?;
    host.deploy("analytics", &fixture::image("crab-analytics")?)
        .await?;
    let site = host
        .call_json(
            "analytics",
            "add_site",
            json!({"domain":"Example.com","name":"Example"}),
        )
        .await?;
    let id = &site["id"];
    for args in [
        json!({"site":id,"page":"/docs?token=secret#fragment","referrer":"https://chatgpt.com/?private=value"}),
        json!({"site":id,"page":"/docs","referrer":"https://www.example.com/home"}),
        json!({"site":id,"page":"/docs","agent":"GPTBot/1.0"}),
    ] {
        assert_eq!(
            host.call_json("analytics", "record", args).await?["kept"],
            true
        );
    }
    assert_eq!(
        host.call_json(
            "analytics",
            "record",
            json!({"site":id,"page":"/docs","agent":"Mozilla/5.0"})
        )
        .await?["kept"],
        false
    );
    let days = host
        .call_json("analytics", "days", json!({"site":id}))
        .await?;
    assert_eq!(days["days"][0]["views"], 2);
    assert_eq!(days["days"][0]["referred"], 1);
    assert_eq!(days["days"][0]["crawls"], 1);
    assert_eq!(days["days"][0]["visitors"], 0);
    assert_eq!(
        host.call_json("analytics", "pages", json!({"site":id}))
            .await?["pages"][0]["page"],
        "/docs"
    );
    let sources = host
        .call_json("analytics", "sources", json!({"site":id}))
        .await?;
    assert_eq!(sources["sources"].as_array().unwrap().len(), 1);
    assert_eq!(sources["sources"][0]["host"], "chatgpt.com");
    host.reopen().await?;
    assert_eq!(
        host.call_json("analytics", "crawlers", json!({"site":id}))
            .await?["crawlers"][0]["requests"],
        1
    );
    let events = host
        .call_json("analytics", "events", json!({"site":id,"limit":1}))
        .await?;
    let next = host
        .call_json(
            "analytics",
            "events",
            json!({"site":id,"after":events["next"]}),
        )
        .await?;
    assert_eq!(next["events"].as_array().unwrap().len(), 2);
    Ok(())
}

#[tokio::test]
async fn retention_bad_ranges_and_site_removal_bound_the_data() -> Result<()> {
    let host = TestHost::open(40 * 86_400_000).await?;
    host.deploy("analytics", &fixture::image("crab-analytics")?)
        .await?;
    let site = host
        .call_json(
            "analytics",
            "add_site",
            json!({"domain":"example.com","name":"Example"}),
        )
        .await?;
    let id = &site["id"];
    host.call_json("analytics", "record", json!({"site":id,"page":"/"}))
        .await?;
    assert!(matches!(
        host.call(
            "analytics",
            "days",
            &serde_json::to_vec(&json!({"site":id,"from":86400000,"to":0}))?
        )
        .await?
        .result,
        Output::Failed(_)
    ));
    host.advance(31 * 86_400_000);
    host.call_json("analytics", "prune", json!({})).await?;
    assert!(
        host.call_json("analytics", "events", json!({"site":id}))
            .await?["events"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let report = host
        .call_json(
            "analytics",
            "days",
            json!({"site":id,"from":40 * 86_400_000_u64}),
        )
        .await?;
    assert_eq!(report["days"][0]["views"], 1);
    host.advance(336 * 86_400_000);
    host.call_json("analytics", "prune", json!({})).await?;
    assert_eq!(
        host.sql("analytics", "SELECT COUNT(*) FROM days", json!([]))
            .await?["rows"][0][0],
        0
    );
    assert_eq!(
        host.call_json("analytics", "remove_site", json!({"site":id}))
            .await?,
        true
    );
    assert!(
        host.call_json("analytics", "sites", json!({}))
            .await?
            .as_array()
            .unwrap()
            .is_empty()
    );
    Ok(())
}

#[tokio::test]
async fn daily_client_keys_deduplicate_visitors_per_site_page_and_source() -> Result<()> {
    let host = TestHost::open(40 * 86_400_000).await?;
    host.deploy("analytics", &fixture::image("crab-analytics")?)
        .await?;
    let site = host
        .call_json(
            "analytics",
            "add_site",
            json!({"domain":"example.com","name":"Example"}),
        )
        .await?;
    let id = &site["id"];
    for (client, page) in [
        ("visitor-a", "/docs"),
        ("visitor-a", "/docs"),
        ("visitor-a", "/about"),
        ("visitor-b", "/docs"),
    ] {
        let ran = host
            .call_with_client(
                "analytics",
                "record",
                &serde_json::to_vec(
                    &json!({"site":id,"page":page,"referrer":"https://chatgpt.com/"}),
                )?,
                client,
            )
            .await?;
        assert!(matches!(ran.result, Output::Done(_)), "{:?}", ran.result);
    }
    let days = host
        .call_json("analytics", "days", json!({"site":id}))
        .await?;
    assert_eq!(days["days"][0]["views"], 4);
    assert_eq!(days["days"][0]["visitors"], 2);
    let pages = host
        .call_json("analytics", "pages", json!({"site":id}))
        .await?;
    assert_eq!(pages["pages"][0]["page"], "/docs");
    assert_eq!(pages["pages"][0]["visitors"], 2);
    assert_eq!(
        host.call_json("analytics", "sources", json!({"site":id}))
            .await?["sources"][0]["visitors"],
        2
    );
    host.advance(86_400_000);
    host.call_json("analytics", "prune", json!({})).await?;
    let ran = host
        .call_with_client(
            "analytics",
            "record",
            &serde_json::to_vec(&json!({"site":id,"page":"/docs"}))?,
            "visitor-a-next-day",
        )
        .await?;
    assert!(matches!(ran.result, Output::Done(_)), "{:?}", ran.result);
    let days = host
        .call_json(
            "analytics",
            "days",
            json!({"site":id,"from":40 * 86_400_000_u64}),
        )
        .await?;
    assert_eq!(days["days"][1]["visitors"], 1);
    Ok(())
}
