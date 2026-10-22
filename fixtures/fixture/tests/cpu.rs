//! A call's CPU time, metered and bounded.

use anyhow::Result;
use berm_api::Output;
use berm_host::test::{self, TestHost};
use std::time::Duration;

#[tokio::test]
async fn a_call_is_metered_the_cpu_it_spent() -> Result<()> {
    let host = TestHost::open(0).await?;
    host.deploy("fixture", &test::build("berm-fixture")?)
        .await?;

    let short = host.call("fixture", "spin", br#"{"rounds":1}"#).await?;
    let long = host
        .call("fixture", "spin", br#"{"rounds":50000000}"#)
        .await?;
    assert!(matches!(long.result, Output::Done(_)));
    assert!(long.meter.cpu > short.meter.cpu);
    assert!(long.meter.cpu > 1_000, "{}µs", long.meter.cpu);
    Ok(())
}

#[tokio::test]
async fn a_call_past_its_cpu_budget_is_stopped() -> Result<()> {
    let host = TestHost::open_with(0, Duration::from_millis(1)).await?;
    host.deploy("fixture", &test::build("berm-fixture")?)
        .await?;

    let ran = host
        .call("fixture", "spin", br#"{"rounds":1000000000000}"#)
        .await;
    let error = format!("{:#}", ran.expect_err("the call is stopped"));
    assert!(
        error.contains("ran past its budget of 1000µs of CPU"),
        "{error}"
    );
    Ok(())
}
