//! A host's written calls and logs as its fleet pulls them: past the cursors
//! it stored, those up to them deleted, ids never reused, and replays and a
//! cron's last run kept past the deletion.

use anyhow::Result;
use berm_api::{
    Output,
    host::{CallPage, Caller, RecordedLog, Records, RecordsPage},
};
use berm_host::test::{self, TestHost};

const FIXTURE: &str = "acct/fixture";

/// 2026-10-12T00:00:00Z.
const NOW: u64 = 1_791_763_200_000;

fn caller() -> Caller {
    Caller {
        id: "acct".to_owned(),
        app: None,
        user: None,
        client: None,
        agent: None,
    }
}

fn page() -> CallPage {
    CallPage {
        service: None,
        project: None,
        tool: None,
        event: None,
        q: None,
        outcome: None,
        since: None,
        until: None,
        before: None,
        limit: 100,
    }
}

async fn fixture() -> Result<TestHost> {
    let host = TestHost::open(NOW).await?;
    host.deploy(FIXTURE, &test::build("berm-fixture")?).await?;
    Ok(host)
}

/// Call `note` on the fixture with `word`, from outside.
async fn note(host: &TestHost, word: &str) -> Result<Output> {
    let args = format!(r#"{{"word":"{word}"}}"#).into_bytes();
    let (_, output) = host
        .host
        .call(FIXTURE, Some(&caller()), "note", args, false)
        .await?;
    Ok(output)
}

async fn records(host: &TestHost, calls: u64, logs: u64, limit: u32) -> Result<Records> {
    host.host.records(RecordsPage { calls, logs, limit }).await
}

/// The cursors that store everything in `records`.
fn stored(records: &Records) -> (u64, u64) {
    (
        records.calls.last().map_or(0, |call| call.call.id),
        records.logs.last().map_or(0, |log| log.id),
    )
}

fn words(logs: &[RecordedLog]) -> Vec<String> {
    logs.iter().map(|log| log.log.data.clone()).collect()
}

#[tokio::test]
async fn records_answers_past_the_cursors_and_deletes_up_to_them() -> Result<()> {
    let host = fixture().await?;
    note(&host, "one").await?;
    note(&host, "two").await?;
    note(&host, "three").await?;
    host.flush().await?;

    let first = records(&host, 0, 0, 2).await?;
    assert_eq!(first.calls.len(), 2);
    assert_eq!(first.logs.len(), 2);
    assert!(first.calls[0].call.id < first.calls[1].call.id);
    assert!(first.logs[0].id < first.logs[1].id);
    for (call, log) in first.calls.iter().zip(&first.logs) {
        assert_eq!(
            (call.scope.as_str(), call.service.as_str()),
            ("acct", "fixture")
        );
        assert_eq!(call.call.tool, "note");
        assert_eq!(call.call.logs, 1);
        assert_eq!(log.log.call, Some(call.call.id));
        assert_eq!(
            (log.scope.as_str(), log.service.as_str()),
            ("acct", "fixture")
        );
    }

    let (calls, logs) = stored(&first);
    let rest = records(&host, calls, logs, 10).await?;
    assert_eq!(rest.calls.len(), 1);
    assert!(rest.calls[0].call.id > calls);
    assert_eq!(rest.calls[0].call.logs, 1);
    assert_eq!(rest.logs.len(), 1);
    assert!(rest.logs[0].id > logs);
    assert!(words(&rest.logs)[0].contains("three"));

    // What was stored is gone from the host, and what was not is still there.
    let (written, _) = host.host.calls(FIXTURE, page()).await?;
    assert_eq!(written.len(), 1);
    assert_eq!(written[0].id, rest.calls[0].call.id);
    assert_eq!(host.logs(FIXTURE, None).await?.len(), 1);

    let (calls, logs) = stored(&rest);
    let none = records(&host, calls, logs, 10).await?;
    assert!(none.calls.is_empty() && none.logs.is_empty());
    assert!(host.host.calls(FIXTURE, page()).await?.0.is_empty());
    assert!(host.logs(FIXTURE, None).await?.is_empty());
    Ok(())
}

#[tokio::test]
async fn ids_are_not_reused_once_deleted() -> Result<()> {
    let mut host = fixture().await?;
    note(&host, "one").await?;
    host.flush().await?;
    let first = records(&host, 0, 0, 10).await?;
    let (calls, logs) = stored(&first);
    assert!(calls > 0 && logs > 0);
    records(&host, calls, logs, 10).await?;

    host.reopen().await?;
    note(&host, "two").await?;
    host.flush().await?;
    let next = records(&host, 0, 0, 10).await?;
    assert_eq!(next.calls.len(), 1);
    assert!(next.calls[0].call.id > calls);
    assert_eq!(next.logs.len(), 1);
    assert!(next.logs[0].id > logs);
    Ok(())
}

#[tokio::test]
async fn a_failed_calls_replay_outlives_its_deletion() -> Result<()> {
    let host = fixture().await?;
    let output = note(&host, "fail").await?;
    assert!(matches!(output, Output::Failed(_)), "{output:?}");
    host.flush().await?;
    let failed = records(&host, 0, 0, 10).await?;
    let id = failed.calls[0].call.id;
    let (calls, logs) = stored(&failed);
    records(&host, calls, logs, 10).await?;
    assert!(host.host.calls(FIXTURE, page()).await?.0.is_empty());

    let input = host.host.call_input(FIXTURE, id).await?;
    assert_eq!(input.tool, "note");
    assert_eq!(input.args, r#"{"word":"fail"}"#);
    assert_eq!(
        input.caller.map(|caller| caller.id).as_deref(),
        Some("acct")
    );
    host.host.call_tape(FIXTURE, id).await?;
    Ok(())
}

#[tokio::test]
async fn a_crons_last_run_outlives_its_deletion() -> Result<()> {
    const FEED: &str = "http://127.0.0.1/feed.xml";
    let host = TestHost::open(NOW).await?;
    host.deploy("feed", &test::build("berm-feed")?).await?;
    host.arrange(
        "GET",
        FEED,
        200,
        r#"<?xml version="1.0"?><rss version="2.0"><channel><title>Feed</title></channel></rss>"#,
    );
    host.host
        .set_cron("feed", "tick", Some("* * * * *".to_owned()))
        .await?;
    let wakes = host.wakes().await?;
    host.set_now(wakes[0].1);
    assert_eq!(host.fire().await?.len(), 1);

    let unsaved = host.host.crons("feed").await?.crons;
    host.flush().await?;
    let written = host.host.crons("feed").await?.crons;
    let last = written[0].last.clone().expect("the tick ran");
    assert_eq!(last.tool, "tick");
    assert_eq!(
        serde_json::to_value(&unsaved[0].last)?,
        serde_json::to_value(&written[0].last)?
    );

    let pulled = records(&host, 0, 0, 10).await?;
    assert_eq!(pulled.calls.len(), 1);
    let (calls, logs) = stored(&pulled);
    records(&host, calls, logs, 10).await?;
    assert!(host.host.calls("feed", page()).await?.0.is_empty());

    let kept = host.host.crons("feed").await?.crons;
    assert_eq!(
        serde_json::to_value(&kept[0].last)?,
        serde_json::to_value(&written[0].last)?
    );
    Ok(())
}
