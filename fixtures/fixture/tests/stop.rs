//! A stopped service: no calls from outside, services or peers, and no wakes,
//! and everything it had waiting runs once it starts.

use anyhow::Result;
use berm_api::{Output, host::Caller};
use berm_host::test::{self, TestHost};

fn caller() -> Caller {
    Caller {
        id: "acct".to_owned(),
        app: None,
        user: None,
        client: None,
        agent: None,
    }
}

async fn deployed(services: &[&str]) -> Result<TestHost> {
    let host = TestHost::open(0).await?;
    let image = test::build("berm-fixture")?;
    for service in services {
        host.deploy(service, &image).await?;
    }
    Ok(host)
}

#[tokio::test]
async fn a_stopped_wake_stays_and_fires_once_started() -> Result<()> {
    let host = deployed(&["acct/fixture", "other/fixture"]).await?;
    host.call("acct/fixture", "tick", br#"{"delay":0}"#).await?;
    host.call("other/fixture", "tick", br#"{"delay":0}"#)
        .await?;

    assert!(host.host().stop("acct/fixture").await?);
    let fired = host.fire().await?;
    assert_eq!(fired.len(), 1);
    assert_eq!(fired[0].0.service, "other/fixture");
    assert_eq!(host.wakes().await?.len(), 2);

    assert!(host.host().start("acct/fixture").await?);
    let fired = host.fire().await?;
    assert!(fired.iter().any(|(wake, _)| wake.service == "acct/fixture"));
    Ok(())
}

#[tokio::test]
async fn a_stopped_service_takes_no_calls_until_started() -> Result<()> {
    let host = deployed(&["acct/fixture", "acct/inner"]).await?;
    assert!(host.host().stop("acct/inner").await?);
    assert!(host.host().runtime("acct/inner").await?.stopped);

    let refused = host
        .host()
        .call(
            "acct/inner",
            Some(&caller()),
            "echo",
            br#"{"query":"hi"}"#.to_vec(),
            false,
        )
        .await
        .expect_err("a call from outside into a stopped service is refused");
    assert_eq!(refused.0.as_u16(), 503);
    let nested = host
        .call("acct/fixture", "nest", br#"{"query":"hi"}"#)
        .await?;
    let Output::Failed(failure) = nested.result else {
        panic!("a call into a stopped service is refused");
    };
    assert!(failure.contains("stopped"), "{failure}");

    assert!(host.host().start("acct/inner").await?);
    assert!(!host.host().runtime("acct/inner").await?.stopped);
    assert!(
        host.host()
            .call(
                "acct/inner",
                Some(&caller()),
                "echo",
                br#"{"query":"hi"}"#.to_vec(),
                false
            )
            .await
            .is_ok()
    );
    assert!(matches!(
        host.call("acct/fixture", "nest", br#"{"query":"hi"}"#)
            .await?
            .result,
        Output::Done(_)
    ));
    Ok(())
}

#[tokio::test]
async fn a_service_deployed_stopped_serves_once_started_and_a_deploy_starts_it() -> Result<()> {
    let host = TestHost::open(0).await?;
    let image = test::build("berm-fixture")?;
    host.pushed("acct/fixture", &image).await?;
    assert!(host.host().stopped("acct/fixture"));
    let caller = caller();
    let call = || {
        host.host().call(
            "acct/fixture",
            Some(&caller),
            "echo",
            br#"{"query":"hi"}"#.to_vec(),
            false,
        )
    };
    assert!(call().await.is_err());

    assert!(host.host().start("acct/fixture").await?);
    assert!(call().await.is_ok());

    assert!(host.host().stop("acct/fixture").await?);
    host.deploy("acct/fixture", &image).await?;
    assert!(!host.host().stopped("acct/fixture"));
    assert!(call().await.is_ok());

    assert!(!host.host().stop("acct/absent").await?);
    Ok(())
}
