//! A scope's memory: its services' images while deployed and its invocations'
//! linear memory while they run, held to its limit; and a tool's budget of
//! memory, which each call reserves and is metered for.

use anyhow::Result;
use berm_api::{Budget, Output};
use berm_host::test::{self, TestHost};
use std::collections::HashMap;

/// A page of linear memory.
const PAGE: u64 = 64 << 10;

#[tokio::test]
async fn loaded_images_count_and_a_call_gives_back_what_it_held() -> Result<()> {
    let host = TestHost::open(0).await?;
    let program = host
        .deploy("acct/fixture", &test::build("berm-fixture")?)
        .await?;
    host.call("acct/fixture", "carry", b"{}").await?;
    let memory = host.host().memory("acct");
    assert_eq!(memory.limit, 0);
    assert_eq!(memory.loaded, program.bytes());
    assert_eq!(memory.running, 0);
    Ok(())
}

#[tokio::test]
async fn a_call_past_the_limit_fails() -> Result<()> {
    let host = TestHost::open(0).await?;
    let program = host
        .deploy("acct/fixture", &test::build("berm-fixture")?)
        .await?;
    host.host()
        .limit(HashMap::from([("acct".to_owned(), program.bytes() + PAGE)]));
    let failed = match host.call("acct/fixture", "carry", b"{}").await {
        Ok(ran) => format!("{:?}", ran.result),
        Err(e) => format!("{e:#}"),
    };
    assert!(failed.contains("past its limit"), "{failed}");
    assert_eq!(host.host().memory("acct").running, 0);

    host.host().limit(HashMap::new());
    host.call("acct/fixture", "carry", b"{}").await?;
    Ok(())
}

#[tokio::test]
async fn a_deploy_whose_images_would_pass_the_limit_is_refused() -> Result<()> {
    let host = TestHost::open(0).await?;
    let image = test::build("berm-fixture")?;
    let bytes = host.deploy("acct/fixture", &image).await?.bytes();
    host.host()
        .limit(HashMap::from([("acct".to_owned(), bytes + PAGE)]));
    host.deploy("acct/fixture", &image).await?;
    let Err(refused) = host.deploy("acct/inner", &image).await else {
        panic!("two images do not fit");
    };
    assert!(
        format!("{refused:#}").contains("past its limit"),
        "{refused:#}"
    );
    assert_eq!(host.host().memory("acct").loaded, bytes);
    host.deploy("other/fixture", &image).await?;
    Ok(())
}

/// `budget` of memory on `tool`, the host's CPU and rows kept.
async fn budget(host: &TestHost, tool: &str, memory: u64) -> Result<()> {
    let defaults = berm_host::Config::default();
    host.host()
        .set_budget(
            "fixture",
            tool,
            Budget {
                cpu: defaults.cpu.as_micros() as u64,
                rows: defaults.rows,
                memory,
            },
        )
        .await
        .map_err(|e| anyhow::anyhow!("{e:?}"))
}

#[tokio::test]
async fn a_call_is_metered_its_budget_not_what_it_used() -> Result<()> {
    let host = TestHost::open(0).await?;
    host.deploy("fixture", &test::build("berm-fixture")?)
        .await?;
    let unset = host.call("fixture", "carry", b"{}").await?.meter;
    budget(&host, "carry", 2 << 20).await?;
    let budgeted = host.call("fixture", "carry", b"{}").await?.meter;
    assert!(unset.memory > 0 && unset.held > 0, "{unset:?}");
    assert_eq!(unset.memory, budgeted.memory);
    assert!(budgeted.held * 8 < unset.held, "{unset:?} {budgeted:?}");
    Ok(())
}

#[tokio::test]
async fn a_call_past_its_memory_budget_fails() -> Result<()> {
    let host = TestHost::open(0).await?;
    host.deploy("fixture", &test::build("berm-fixture")?)
        .await?;
    budget(&host, "carry", PAGE).await?;
    let trapped = host.call("fixture", "carry", b"{}").await.unwrap_err();
    assert!(
        format!("{trapped:#}").contains(&format!("budget of {PAGE}")),
        "{trapped:#}"
    );
    Ok(())
}

#[tokio::test]
async fn a_memory_budget_past_the_hosts_is_refused() -> Result<()> {
    let host = TestHost::open(0).await?;
    host.deploy("fixture", &test::build("berm-fixture")?)
        .await?;
    let past = berm_host::Config::default().memory as u64 + 1;
    assert!(budget(&host, "carry", past).await.is_err());
    Ok(())
}

#[tokio::test]
async fn a_call_lowers_its_memory_once_and_is_metered_the_lower_after() -> Result<()> {
    let host = TestHost::open(0).await?;
    host.deploy("fixture", &test::build("berm-fixture")?)
        .await?;
    let kept = host
        .call("fixture", "lowered", br#"{"memory":0,"rounds":20000000}"#)
        .await?;
    let lowered = host
        .call(
            "fixture",
            "lowered",
            format!(r#"{{"memory":{},"rounds":20000000}}"#, 2 << 20).as_bytes(),
        )
        .await?;
    assert!(
        matches!(&lowered.result, Output::Done(refused) if refused == "true"),
        "{:?}",
        lowered.result
    );
    assert!(
        lowered.meter.held * 4 < kept.meter.held,
        "{:?} {:?}",
        kept.meter,
        lowered.meter
    );
    Ok(())
}

#[tokio::test]
async fn a_call_cannot_raise_its_memory_or_lower_it_below_what_it_holds() -> Result<()> {
    let host = TestHost::open(0).await?;
    host.deploy("fixture", &test::build("berm-fixture")?)
        .await?;
    budget(&host, "lowered", 4 << 20).await?;
    for memory in [8 << 20, PAGE] {
        let ran = host
            .call(
                "fixture",
                "lowered",
                format!(r#"{{"memory":{memory},"rounds":0}}"#).as_bytes(),
            )
            .await?;
        assert!(matches!(ran.result, Output::Failed(_)), "{:?}", ran.result);
    }
    Ok(())
}
