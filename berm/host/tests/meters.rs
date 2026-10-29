//! What a service used, read the same whether it is written yet or not: its
//! runs as its listing counts them, its use as `live` sums it, its logs as
//! numbered, and its calls as a page's filters narrow them.

use anyhow::Result;
use berm_api::host::{CallPage, Caller};
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

fn page(tool: Option<&str>) -> CallPage {
    CallPage {
        service: None,
        project: None,
        tool: tool.map(str::to_owned),
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
async fn note(host: &TestHost, word: &str) -> Result<()> {
    let args = format!(r#"{{"word":"{word}"}}"#).into_bytes();
    host.host
        .call(FIXTURE, Some(&caller()), "note", args, false)
        .await?;
    Ok(())
}

/// The fixture's runs as its listing counts them, and its bytes logged in
/// total as `live` sums them.
async fn used(host: &TestHost) -> Result<(u64, u64)> {
    let listing = host.host.listing(FIXTURE).await?;
    let live = host.host.live(FIXTURE).await?;
    Ok((listing.served.runs, live.logged.total))
}

#[tokio::test]
async fn runs_and_use_read_alike_written_or_not() -> Result<()> {
    let mut host = fixture().await?;
    note(&host, "one").await?;
    note(&host, "two").await?;
    let (runs, logged) = used(&host).await?;
    assert_eq!(runs, 2);
    assert!(logged > 0);

    host.flush().await?;
    assert_eq!(used(&host).await?, (runs, logged));
    let listed = host.host.listings("acct").await?;
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].served.runs, runs);

    note(&host, "three").await?;
    let (more, logged_more) = used(&host).await?;
    assert_eq!(more, 3);
    assert!(logged_more > logged);
    host.flush().await?;
    host.reopen().await?;
    assert_eq!(used(&host).await?, (more, logged_more));
    Ok(())
}

#[tokio::test]
async fn logs_written_in_one_batch_number_in_order() -> Result<()> {
    let host = fixture().await?;
    for word in ["one", "two", "three"] {
        note(&host, word).await?;
    }
    host.flush().await?;
    note(&host, "four").await?;
    host.flush().await?;
    let logs = host.logs(FIXTURE, None).await?;
    let seqs: Vec<u64> = logs.iter().map(|log| log.seq).collect();
    assert_eq!(seqs, [1, 2, 3, 4]);
    for (log, word) in logs.iter().zip(["one", "two", "three", "four"]) {
        assert!(log.data.contains(word));
    }
    Ok(())
}

#[tokio::test]
async fn a_tool_filter_narrows_calls_and_their_counts() -> Result<()> {
    let host = fixture().await?;
    note(&host, "one").await?;
    note(&host, "two").await?;
    host.flush().await?;
    note(&host, "three").await?;

    let (noted, _) = host.host.scope_calls("acct", page(Some("note"))).await?;
    assert_eq!(noted.len(), 3);
    let (none, _) = host.host.scope_calls("acct", page(Some("nothing"))).await?;
    assert!(none.is_empty());
    let (all, _) = host.host.calls(FIXTURE, page(None)).await?;
    assert_eq!(all.len(), 3);

    let stats = host.host.call_stats(FIXTURE, page(Some("note"))).await?;
    assert_eq!(stats.tools.get("note"), Some(&3));
    let counted: u64 = stats
        .buckets
        .iter()
        .flat_map(|bucket| bucket.outcomes.values())
        .sum();
    assert_eq!(counted, 3);
    Ok(())
}

/// What every read of the fixture counts: runs listed, bytes logged, uses,
/// calls paged, the seqs of its logs, and calls metered.
async fn counted(host: &TestHost) -> Result<(u64, u64, i64, usize, Vec<u64>, u64)> {
    let (runs, logged) = used(host).await?;
    let uses = host.host.usage(FIXTURE, None).await?;
    let (calls, _) = host.host.calls(FIXTURE, page(None)).await?;
    let logs = host.logs(FIXTURE, None).await?;
    let meters = host.host.meters(FIXTURE).await?;
    Ok((
        runs,
        logged,
        uses.iter().map(|used| used.calls).sum(),
        calls.len(),
        logs.iter().map(|log| log.seq).collect(),
        meters.iter().map(|metered| metered.calls).sum(),
    ))
}

#[tokio::test]
async fn a_read_during_a_flush_counts_each_call_once() -> Result<()> {
    let host = fixture().await?;
    note(&host, "one").await?;
    host.flush().await?;
    note(&host, "two").await?;
    note(&host, "three").await?;
    let before = counted(&host).await?;
    assert_eq!(before.3, 3);

    // The writer held: the flush has taken what was pending and written none of it.
    let (held, holding) = tokio::sync::oneshot::channel();
    let (release, released) = std::sync::mpsc::channel::<()>();
    let store = host.host.store.clone();
    let writer = tokio::spawn(async move {
        store
            .run(move |_| {
                held.send(()).ok();
                released.recv().ok();
                Ok(())
            })
            .await
    });
    holding.await?;
    let (flushed, during) = tokio::join!(host.flush(), async {
        let during = counted(&host).await;
        release.send(()).ok();
        during
    });
    writer.await??;
    flushed?;
    assert_eq!(during?, before);
    assert_eq!(counted(&host).await?, before);

    // Written, and still held in memory.
    note(&host, "four").await?;
    let before = counted(&host).await?;
    assert_eq!(before.3, 4);
    let landing = host.landing().await;
    let (flushed, during) = tokio::join!(host.flush(), async {
        loop {
            let written: i64 = host
                .host
                .store
                .read(|conn| {
                    conn.query_row(
                        "SELECT COUNT(*) FROM flushes WHERE generation = 3",
                        (),
                        |row| row.get(0),
                    )
                })
                .await?;
            if written == 4 {
                break;
            }
            tokio::task::yield_now().await;
        }
        let during = counted(&host).await;
        drop(landing);
        during
    });
    flushed?;
    assert_eq!(during?, before);
    assert_eq!(counted(&host).await?, before);
    Ok(())
}
