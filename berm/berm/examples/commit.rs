//! What a chain leaves for its host to commit.
//!
//! The reference guest's `note` writes and emits; `relay` calls `note` on
//! `inner` and carries on whether or not it failed. A chain that returns a
//! result hands back every write and log it kept, one that fails hands back
//! nothing, and a failed nested call takes its own with it. `peek` is a query:
//! everything it tries to leave behind is refused, nested calls included.
//!
//! ```sh
//! cargo run -p crab-agent -- build fixtures/fixture
//! cargo run --release --example commit -p berm
//! ```

use anyhow::{Context, Result, ensure};
use berm::{
    Berm, Calls, Changes, Config, Engine, Output,
    syscall::{after, emit, store},
};
use parking_lot::Mutex;
use std::{collections::HashMap, fs, path::PathBuf, sync::Arc};

const GUEST: &str = "target/berm/berm_fixture.wasm";

type Committed = Arc<Mutex<HashMap<(String, String), Vec<u8>>>>;

#[tokio::main]
async fn main() -> Result<()> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .context("no workspace root")?
        .to_path_buf();
    let image = fs::read(root.join(GUEST)).with_context(|| {
        format!(
            "build the guest first: cargo run -p crab-agent -- build fixtures/fixture ({GUEST})"
        )
    })?;

    let engine = Engine::new(&Config {
        cache_dir: Some(std::env::temp_dir().join("berm-commit")),
        ..Config::default()
    })?;
    let committed = Committed::default();
    let reading = committed.clone();
    let mut syscalls = store::programs(
        move |at, key| {
            std::future::ready(Ok(reading
                .lock()
                .get(&(at.program.to_string(), key))
                .cloned()))
        },
        |_, _, _| async { Ok(false) },
    );
    syscalls.push(emit::programs(|_, _| async { Ok(false) }));
    syscalls.push(after::programs(|_, _| async { Ok(()) }));
    let berm = Berm::new(&engine, Calls::by_name(Calls::DEFAULT_DEPTH), syscalls);
    berm.deploy("outer", &image)?;
    berm.deploy("inner", &image)?;

    // Kept: the write is read back inside the chain, and both reach the host.
    let outcome = berm
        .call(None, "outer", "note", r#"{"word":"one"}"#)
        .await?;
    ensure!(
        outcome.result == Output::Done(r#""one""#.to_owned()),
        "{:?}",
        outcome.result
    );
    ensure!(
        said(&outcome.changes)
            == [
                "write outer note one",
                r#"log outer [Noted] {"word":"one","bytes":3}"#
            ],
        "{:?}",
        outcome.changes
    );
    commit(&committed, &outcome.changes);
    println!("kept:     {:?}", said(&outcome.changes));

    // Failed: nothing reaches the host.
    let outcome = berm
        .call(None, "outer", "note", r#"{"word":"fail"}"#)
        .await?;
    ensure!(
        matches!(outcome.result, Output::Failed(_)),
        "{:?}",
        outcome.result
    );
    ensure!(said(&outcome.changes).is_empty(), "{:?}", outcome.changes);
    println!("failed:   {:?}", said(&outcome.changes));

    // A failed nested call takes its own; the caller's log stays, and its key
    // still reads what was committed.
    let outcome = berm
        .call(None, "outer", "relay", r#"{"word":"fail"}"#)
        .await?;
    ensure!(
        outcome.result == Output::Done(r#""one""#.to_owned()),
        "{:?}",
        outcome.result
    );
    ensure!(
        said(&outcome.changes) == ["log outer [relay] fail"],
        "{:?}",
        outcome.changes
    );
    println!("reverted: {:?}", said(&outcome.changes));

    // A nested call that returns keeps its own, in the order it emitted them.
    let outcome = berm
        .call(None, "outer", "relay", r#"{"word":"two"}"#)
        .await?;
    ensure!(
        said(&outcome.changes)
            == [
                "write inner note two",
                r#"log inner [Noted] {"word":"two","bytes":3}"#,
                "log outer [relay] two",
            ],
        "{:?}",
        outcome.changes
    );
    println!("nested:   {:?}", said(&outcome.changes));

    // A query reads what is committed and leaves nothing: its writes, logs and
    // wakes are refused, and so are those of a tool it calls.
    let outcome = berm.call(None, "outer", "peek", "").await?;
    let answered = Result::from(outcome.result.clone()).map_err(anyhow::Error::msg)?;
    let peek: serde_json::Value = serde_json::from_str(&answered)?;
    ensure!(peek["note"] == "one", "{peek}");
    for attempt in ["set", "emit", "after", "call"] {
        let told = peek["attempts"][attempt].as_str().unwrap_or_default();
        ensure!(told.starts_with("a query cannot"), "{attempt}: {told}");
    }
    ensure!(said(&outcome.changes).is_empty(), "{:?}", outcome.changes);
    println!("query:    {peek}");

    println!("\nok");
    Ok(())
}

/// Each write, then each log, as one line.
fn said(changes: &Changes) -> Vec<String> {
    let writes = changes.writes.iter().map(|write| {
        format!(
            "write {} {} {}",
            write.program,
            write.key,
            String::from_utf8_lossy(&write.value)
        )
    });
    let logs = changes.logs.iter().map(|log| {
        format!(
            "log {} [{}] {}",
            log.program,
            log.topics.join(","),
            log.data
        )
    });
    writes.chain(logs).collect()
}

fn commit(committed: &Committed, changes: &Changes) {
    let mut committed = committed.lock();
    for write in &changes.writes {
        committed.insert(
            (write.program.to_string(), write.key.clone()),
            write.value.clone(),
        );
    }
}
