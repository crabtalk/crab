//! A failed call's tape: recorded on the host, replayed in a runtime of its own
//! with nothing reachable, and refused where the call asks differently.

use anyhow::Result;
use berm::{
    Berm, Calls, Config, Engine, Output, chain,
    tape::{Answer, Tape, Taping},
};
use berm_api::host::{CallPage, Caller, Outcome};
use berm_host::test::{self, TestHost};
use std::{sync::Arc, time::Duration};

const URL: &str = "http://127.0.0.1/status";

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
        limit: 10,
    }
}

/// Replay `tape` as a call of `tool` with `args`, in a runtime that reaches
/// nothing.
async fn replay(image: &[u8], tool: &str, args: &str, tape: Tape) -> Result<Output> {
    let engine = Engine::new(&Config::default())?;
    let berm = Berm::new(&engine, Calls::by_name(Calls::DEFAULT_DEPTH), Vec::new());
    let program = berm.deploy("fixture", image)?;
    let outcome = program
        .call_as(
            None,
            None,
            None,
            tool,
            args,
            Arc::default(),
            chain::next(),
            Taping::Replay {
                tape,
                through: Vec::new(),
            },
        )
        .await?;
    Ok(outcome.result)
}

#[tokio::test]
async fn a_failed_call_replays_to_the_same_failure() -> Result<()> {
    let host = TestHost::open(0).await?;
    let image = test::build("berm-fixture")?;
    host.deploy("acct/fixture", &image).await?;
    host.arrange("GET", URL, 503, "busy");
    let args = format!(r#"{{"url":"{URL}"}}"#);
    let (_, output) = host
        .host
        .call(
            "acct/fixture",
            Some(&caller()),
            "flaky",
            args.clone().into_bytes(),
            false,
        )
        .await?;
    let Output::Failed(failure) = output else {
        panic!("{output:?}")
    };

    let (calls, _) = host.host.calls("acct/fixture", page()).await?;
    let tape = Tape::try_from(&host.host.call_tape("acct/fixture", calls[0].id).await?[..])?;
    assert!(tape.complete);
    assert_eq!(tape.entries.len(), 2);
    assert!(matches!(tape.entries[1].answer, Answer::Done(ref bytes) if bytes.len() == 4));
    assert_eq!(Tape::try_from(&Vec::from(&tape)[..])?, tape);

    // Nothing is reachable here: the fetch and the random bytes come off the tape.
    let replayed = replay(&image, "flaky", &args, tape.clone()).await?;
    assert_eq!(replayed, Output::Failed(failure));

    let other = replay(&image, "flaky", r#"{"url":"http://127.0.0.1/other"}"#, tape)
        .await
        .unwrap_err();
    assert!(
        format!("{other:#}").contains("diverged at syscall 1"),
        "{other:#}"
    );
    Ok(())
}

#[tokio::test]
async fn a_done_call_keeps_no_tape() -> Result<()> {
    let host = TestHost::open(0).await?;
    host.deploy("acct/fixture", &test::build("berm-fixture")?)
        .await?;
    host.host
        .call("acct/fixture", Some(&caller()), "random", Vec::new(), false)
        .await?;
    let (calls, _) = host.host.calls("acct/fixture", page()).await?;
    assert!(
        host.host
            .call_tape("acct/fixture", calls[0].id)
            .await
            .is_err()
    );
    Ok(())
}

#[tokio::test]
async fn a_trapped_call_is_recorded_as_failed_with_its_input_and_tape() -> Result<()> {
    let host = TestHost::open_with(0, Duration::from_millis(5)).await?;
    host.deploy("acct/fixture", &test::build("berm-fixture")?)
        .await?;
    let trapped = host
        .host
        .call(
            "acct/fixture",
            Some(&caller()),
            "spin",
            br#"{"rounds":10000000000}"#.to_vec(),
            false,
        )
        .await;
    assert!(trapped.is_err());
    let (calls, _) = host.host.calls("acct/fixture", page()).await?;
    assert_eq!(calls[0].outcome, Outcome::Failed);
    let input = host.host.call_input("acct/fixture", calls[0].id).await?;
    assert_eq!(input.tool, "spin");
    host.host.call_tape("acct/fixture", calls[0].id).await?;
    Ok(())
}
