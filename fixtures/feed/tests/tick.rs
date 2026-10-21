//! `crabkit-lake`'s tick in berm-feed, against an arranged RSS feed: items stored
//! once, undated or at the watermark included, a failed fetch logged and
//! retried, and the items read back in order.

use anyhow::Result;
use berm_api::Output;
use berm_host::test::{self, TestHost};
use serde_json::Value;

const FEED: &str = "http://127.0.0.1/feed.xml";

/// 2026-10-12T00:00:00Z.
const NOW: u64 = 1_791_763_200_000;

/// The feed's refetch interval.
const REFETCH: u64 = 60 * 1000;

/// An RSS feed of `items`, each `(guid, pubDate)`, as listed.
fn feed(items: &[(&str, &str)]) -> String {
    let items: String = items
        .iter()
        .map(|(guid, date)| {
            format!(
                "<item><title>Post {guid}</title><link>https://example.com/{guid}</link>\
                 <guid>{guid}</guid><pubDate>{date}</pubDate></item>"
            )
        })
        .collect();
    format!(
        r#"<?xml version="1.0"?><rss version="2.0"><channel><title>Feed</title>{items}</channel></rss>"#
    )
}

const B: (&str, &str) = ("b", "Sun, 11 Oct 2026 12:00:00 GMT");
const A: (&str, &str) = ("a", "Sun, 11 Oct 2026 06:00:00 GMT");
const C: (&str, &str) = ("c", "Sun, 11 Oct 2026 18:00:00 GMT");

async fn deployed() -> Result<TestHost> {
    let host = TestHost::open(NOW).await?;
    host.deploy("feed", &test::build("berm-feed", "feed")?)
        .await?;
    Ok(host)
}

async fn count(host: &TestHost) -> Result<u64> {
    let answer = host
        .sql(
            "feed",
            "SELECT COUNT(*) FROM lake_items",
            serde_json::json!([]),
        )
        .await?;
    Ok(answer["rows"][0][0].as_u64().unwrap_or_default())
}

async fn tick(host: &TestHost) -> Result<()> {
    let ran = host.call("feed", "tick", b"{}").await?;
    assert!(matches!(ran.result, Output::Done(_)), "{:?}", ran.result);
    Ok(())
}

/// The `data` of each log under `topic`, oldest first.
async fn logged(host: &TestHost, topic: &str) -> Result<Vec<Value>> {
    host.logs("feed", Some(topic))
        .await?
        .iter()
        .map(|log| Ok(serde_json::from_str(&log.data)?))
        .collect()
}

#[tokio::test]
async fn the_manifest_carries_lake_and_its_events() -> Result<()> {
    let host = TestHost::open(NOW).await?;
    let program = host
        .deploy("feed", &test::build("berm-feed", "feed")?)
        .await?;
    let manifest = program.manifest();
    assert_eq!(manifest.kits.len(), 1);
    assert_eq!(manifest.kits[0].name, "crabkit-lake");
    for syscall in ["berm.get", "berm.set", "berm.sql"] {
        assert!(manifest.deps.syscalls.contains_key(syscall), "{syscall}");
    }
    assert_eq!(manifest.deps.syscalls["berm.http.fetch"], ["127.0.0.1"]);
    let events: Vec<&str> = manifest
        .events
        .iter()
        .map(|event| event.name.as_str())
        .collect();
    assert_eq!(events, ["FetchFailed", "Fetched"]);
    Ok(())
}

#[tokio::test]
async fn a_tick_stores_new_items_once() -> Result<()> {
    let host = deployed().await?;
    host.arrange("GET", FEED, 200, feed(&[B, A]));
    // As `[crons]` in a berm.toml would set it.
    host.host()
        .set_cron("feed", "tick", Some("* * * * *".to_owned()))
        .await?;
    let wakes = host.wakes().await?;
    assert_eq!(wakes.len(), 1);
    assert_eq!(wakes[0].0.tool, "tick");
    host.set_now(wakes[0].1);

    let fired = host.fire().await?;
    assert_eq!(fired.len(), 1);
    assert!(
        matches!(fired[0].1.result, Output::Done(_)),
        "{:?}",
        fired[0].1.result
    );
    assert_eq!(count(&host).await?, 2);

    host.advance(REFETCH);
    assert_eq!(host.fire().await?.len(), 1);
    assert_eq!(host.fetched().len(), 2);
    assert_eq!(count(&host).await?, 2);

    let fetched = logged(&host, "Fetched").await?;
    assert_eq!(fetched.len(), 2);
    assert_eq!(fetched[0]["feed"], FEED);
    assert_eq!(
        (&fetched[0]["shown"], &fetched[0]["new"]),
        (&2.into(), &2.into())
    );
    assert_eq!(
        (&fetched[1]["shown"], &fetched[1]["new"]),
        (&2.into(), &0.into())
    );
    Ok(())
}

#[tokio::test]
async fn a_failed_fetch_is_logged_and_retried_when_next_due() -> Result<()> {
    let host = deployed().await?;
    host.arrange("GET", FEED, 503, "unavailable");
    tick(&host).await?;
    let failed = logged(&host, "FetchFailed").await?;
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0]["of"], FEED);
    assert_eq!(failed[0]["error"], format!("{FEED} answered 503"));
    assert_eq!(count(&host).await?, 0);

    // Not due again until the refetch interval passes.
    host.arrange("GET", FEED, 200, feed(&[B, A]));
    tick(&host).await?;
    assert_eq!(host.fetched().len(), 1);

    host.advance(REFETCH);
    tick(&host).await?;
    assert_eq!(host.fetched().len(), 2);
    assert_eq!(count(&host).await?, 2);
    assert_eq!(logged(&host, "FetchFailed").await?.len(), 1);
    Ok(())
}

#[tokio::test]
async fn items_read_back_in_the_order_stored() -> Result<()> {
    let host = deployed().await?;
    host.arrange("GET", FEED, 200, feed(&[B, A]));
    tick(&host).await?;
    host.arrange("GET", FEED, 200, feed(&[C, B, A]));
    host.advance(REFETCH);
    tick(&host).await?;

    let ran = host.call("feed", "posts", b"{}").await?;
    let Output::Done(page) = ran.result else {
        panic!("posts runs: {:?}", ran.result);
    };
    let page: Value = serde_json::from_str(&page)?;
    let ids: Vec<&str> = page["posts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|post| post["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["b", "a", "c"]);
    assert_eq!(page["next"], 3);
    assert_eq!(page["posts"][0]["link"], "https://example.com/b");
    assert_eq!(page["posts"][2]["posted"], 1_791_741_600_000u64);

    let ran = host
        .call("feed", "posts", br#"{"after":1,"limit":1}"#)
        .await?;
    let Output::Done(page) = ran.result else {
        panic!("posts runs: {:?}", ran.result);
    };
    let page: Value = serde_json::from_str(&page)?;
    assert_eq!(page["posts"].as_array().map(Vec::len), Some(1));
    assert_eq!(page["posts"][0]["id"], "a");
    assert_eq!(page["next"], 2);
    Ok(())
}

#[tokio::test]
async fn an_undated_item_is_stored_once() -> Result<()> {
    let host = deployed().await?;
    // An empty pubDate leaves the item undated.
    host.arrange("GET", FEED, 200, feed(&[("u", "")]));
    tick(&host).await?;
    assert_eq!(count(&host).await?, 1);

    host.advance(REFETCH);
    tick(&host).await?;
    assert_eq!(count(&host).await?, 1);
    let fetched = logged(&host, "Fetched").await?;
    assert_eq!(fetched[0]["new"], 1);
    assert_eq!(fetched[1]["new"], 0);
    Ok(())
}

#[tokio::test]
async fn an_item_posted_at_the_watermark_is_stored() -> Result<()> {
    let host = deployed().await?;
    host.arrange("GET", FEED, 200, feed(&[B, A]));
    tick(&host).await?;
    host.arrange("GET", FEED, 200, feed(&[("d", B.1), B, A]));
    host.advance(REFETCH);
    tick(&host).await?;
    assert_eq!(count(&host).await?, 3);
    let fetched = logged(&host, "Fetched").await?;
    assert_eq!(fetched[1]["new"], 1);
    Ok(())
}

#[tokio::test]
async fn an_undated_item_after_a_dated_one_is_stored() -> Result<()> {
    let host = deployed().await?;
    host.arrange("GET", FEED, 200, feed(&[B, A]));
    tick(&host).await?;
    host.arrange("GET", FEED, 200, feed(&[("u", ""), B, A]));
    host.advance(REFETCH);
    tick(&host).await?;
    assert_eq!(count(&host).await?, 3);
    let fetched = logged(&host, "Fetched").await?;
    assert_eq!(fetched[1]["new"], 1);
    Ok(())
}
