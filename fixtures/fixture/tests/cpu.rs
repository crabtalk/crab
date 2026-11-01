//! A call's CPU time, metered and bounded.

use anyhow::Result;
use berm::{Berm, Calls, Callsite, Config, Engine, Meters, Syscall, abi, chain, tape::Taping};
use berm_api::{Budget, Output};
use berm_build::fixture;
use berm_host::test::TestHost;
use std::{sync::Arc, thread, time::Duration};
use tokio::runtime::Builder;

#[tokio::test]
async fn a_call_is_metered_the_cpu_it_spent() -> Result<()> {
    let host = TestHost::open(0).await?;
    host.deploy("fixture", &fixture::image("berm-fixture")?)
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
async fn a_call_past_its_cpu_budget_is_stopped_on_a_fresh_thread() -> Result<()> {
    let defaults = berm_host::Config::default();
    let host = TestHost::open_with(
        0,
        berm_host::Config {
            budget: Budget {
                cpu: 1_000,
                ..defaults.budget
            },
            ..defaults
        },
    )
    .await?;
    host.deploy("fixture", &fixture::image("berm-fixture")?)
        .await?;

    let ran = thread::spawn(move || -> Result<_> {
        Builder::new_current_thread()
            .enable_all()
            .build()?
            .block_on(host.call("fixture", "spin", br#"{"rounds":1000000000000}"#))
    })
    .join()
    .expect("the CPU-budget worker panicked");
    let error = format!("{:#}", ran.expect_err("the call is stopped"));
    assert!(
        error.contains("ran past its budget of 1000µs of CPU"),
        "{error}"
    );
    Ok(())
}

#[tokio::test]
async fn time_asleep_inside_a_host_call_is_not_charged_as_cpu() -> Result<()> {
    let image = fixture::image("berm-fixture")?;
    let engine = Engine::new(&Config::default())?;
    let get = Syscall {
        name: abi::GET.to_owned(),
        call: Arc::new(|_: Callsite, _: Vec<u8>| {
            Box::pin(async {
                thread::sleep(Duration::from_millis(200));
                Ok(Vec::new())
            })
        }),
    };
    let service = Berm::new(&engine, Calls::by_name(0), vec![get]).deploy("fixture", &image)?;
    let meters = Arc::new(Meters::default());
    let called = service
        .call_as(
            None,
            None,
            None,
            "wait",
            b"{}".to_vec(),
            meters.clone(),
            chain::next(),
            Taping::Off,
        )
        .await?;
    assert_eq!(called.result, Output::Done(r#""unset""#.into()));
    let cpu: u64 = meters.take().values().map(|used| used.cpu).sum();
    assert!(
        cpu < 100_000,
        "{cpu}µs of CPU charged for a sleeping host call"
    );
    Ok(())
}
