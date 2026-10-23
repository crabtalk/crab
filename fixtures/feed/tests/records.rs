//! `crabkit-lake`'s records in berm-feed, against an arranged feed of records: a
//! version stored for each record new to the feed or whose values changed,
//! and the versions read back.

use anyhow::Result;
use berm_api::Output;
use berm_host::test::{self, TestHost};
use serde_json::{Value, json};

const FEED: &str = "http://127.0.0.1/feed.xml";
const LIST: &str = "http://127.0.0.1/list.xml";

/// 2026-10-12T00:00:00Z.
const NOW: u64 = 1_791_763_200_000;

/// The feed's refetch interval.
const REFETCH: u64 = 60 * 1000;

/// A feed of `records`, each `(guid, title)`.
fn list(records: &[(&str, &str)]) -> String {
    let items: String = records
        .iter()
        .map(|(guid, title)| {
            format!(
                "<item><title>{title}</title><link>https://example.com/{guid}</link>\
                 <guid>{guid}</guid></item>"
            )
        })
        .collect();
    format!(r#"<?xml version="1.0"?><rss version="2.0"><channel>{items}</channel></rss>"#)
}

async fn call(host: &TestHost, tool: &str, args: Value) -> Result<Value> {
    let ran = host
        .call("feed", tool, serde_json::to_vec(&args)?.as_slice())
        .await?;
    let Output::Done(result) = ran.result else {
        panic!("{tool} runs: {:?}", ran.result);
    };
    Ok(match result.as_str() {
        "" => Value::Null,
        result => serde_json::from_str(result)?,
    })
}

async fn count(host: &TestHost) -> Result<u64> {
    let answer = host
        .sql("feed", "SELECT COUNT(*) FROM lake_records", json!([]))
        .await?;
    Ok(answer["rows"][0][0].as_u64().unwrap_or_default())
}

#[tokio::test]
async fn a_version_is_stored_when_a_record_is_new_or_changed() -> Result<()> {
    let host = TestHost::open(NOW).await?;
    host.deploy("feed", &test::build("berm-feed")?).await?;
    host.arrange("GET", FEED, 200, list(&[]));
    host.arrange("GET", LIST, 200, list(&[("a", "Acme"), ("b", "Beta")]));
    assert_eq!(call(&host, "list", json!({"url": LIST})).await?, true);
    assert_eq!(call(&host, "list", json!({"url": LIST})).await?, false);

    call(&host, "tick", json!({})).await?;
    assert_eq!(count(&host).await?, 2);

    // Unchanged values store nothing.
    host.advance(REFETCH);
    call(&host, "tick", json!({})).await?;
    assert_eq!(count(&host).await?, 2);

    host.arrange("GET", LIST, 200, list(&[("a", "Acme Inc"), ("b", "Beta")]));
    host.advance(REFETCH);
    call(&host, "tick", json!({})).await?;
    assert_eq!(count(&host).await?, 3);

    let record = call(&host, "record", json!({"id": "a"})).await?;
    assert_eq!(record["records"].as_array().map(Vec::len), Some(1));
    assert_eq!(record["records"][0]["feed"], LIST);
    assert_eq!(record["records"][0]["post"]["title"], "Acme Inc");
    assert_eq!(record["records"][0]["since"], NOW + 2 * REFETCH);

    let versions = call(&host, "versions", json!({"id": "a"})).await?;
    let titles: Vec<&str> = versions["records"]
        .as_array()
        .unwrap()
        .iter()
        .map(|version| version["post"]["title"].as_str().unwrap())
        .collect();
    assert_eq!(titles, ["Acme", "Acme Inc"]);
    assert_eq!(versions["records"][0]["since"], NOW);

    let fetched: Vec<Value> = host
        .logs("feed", Some("Fetched"))
        .await?
        .iter()
        .map(|log| serde_json::from_str(&log.data))
        .collect::<Result<_, _>>()?;
    let new: Vec<&Value> = fetched
        .iter()
        .filter(|fetched| fetched["feed"] == LIST)
        .map(|fetched| &fetched["new"])
        .collect();
    assert_eq!(new, [&json!(2), &json!(0), &json!(1)]);
    Ok(())
}
