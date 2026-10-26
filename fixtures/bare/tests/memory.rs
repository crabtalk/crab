//! A tool's budget of memory, and the memory a call holds.

use anyhow::Result;
use berm_api::Budget;
use berm_host::test::{self, TestHost};

#[tokio::test]
async fn a_call_holds_its_memory_for_as_long_as_it_runs() -> Result<()> {
    let host = TestHost::open(0).await?;
    host.deploy("bare", &test::build("berm-bare")?).await?;
    let ran = host.call("bare", "vouch", b"{}").await?;
    assert!(ran.meter.memory > 0, "{:?}", ran.meter);
    assert!(ran.meter.held > 0, "{:?}", ran.meter);
    Ok(())
}

#[tokio::test]
async fn a_call_past_its_memory_budget_fails() -> Result<()> {
    let host = TestHost::open(0).await?;
    host.deploy("bare", &test::build("berm-bare")?).await?;
    let defaults = berm_host::Config::default();
    host.host()
        .set_budget(
            "bare",
            "vouch",
            Budget {
                cpu: defaults.cpu.as_micros() as u64,
                rows: defaults.rows,
                memory: 64 << 10,
            },
        )
        .await
        .map_err(|e| anyhow::anyhow!("{e:?}"))?;
    let trapped = host.call("bare", "vouch", b"{}").await.unwrap_err();
    assert!(
        format!("{trapped:#}").contains("budget of 65536"),
        "{trapped:#}"
    );
    Ok(())
}

#[tokio::test]
async fn a_memory_budget_past_the_hosts_is_refused() -> Result<()> {
    let host = TestHost::open(0).await?;
    host.deploy("bare", &test::build("berm-bare")?).await?;
    let defaults = berm_host::Config::default();
    let set = host
        .host()
        .set_budget(
            "bare",
            "vouch",
            Budget {
                cpu: defaults.cpu.as_micros() as u64,
                rows: defaults.rows,
                memory: defaults.memory as u64 + 1,
            },
        )
        .await;
    assert!(set.is_err());
    Ok(())
}
