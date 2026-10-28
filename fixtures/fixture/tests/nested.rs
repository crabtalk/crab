//! A run's calls past its first: each numbered in the order it began, under
//! the call that made it, as held and as written.

use anyhow::Result;
use berm_api::host::{CallPage, Caller, Outcome};
use berm_host::test::{self, TestHost};

#[tokio::test]
async fn a_run_keeps_the_calls_it_made() -> Result<()> {
    let host = TestHost::open(0).await?;
    let image = test::build("berm-fixture")?;
    for service in ["acct/fixture", "acct/inner"] {
        host.deploy(service, &image).await?;
    }
    let caller = Caller {
        id: "acct".to_owned(),
        app: None,
        user: None,
        client: None,
        agent: None,
    };
    for tool in ["nest", "echo"] {
        host.host()
            .call(
                "acct/fixture",
                Some(&caller),
                tool,
                br#"{"query":"hi"}"#.to_vec(),
                false,
            )
            .await?;
    }
    let page = CallPage {
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
    };

    for written in [false, true] {
        if written {
            host.flush().await?;
        }
        let (calls, _) = host.host().calls("acct/fixture", page.clone()).await?;
        let [echo, nest] = calls.as_slice() else {
            panic!("two runs, got {}", calls.len());
        };
        assert!(
            host.host()
                .call_nested("acct/fixture", echo.id)
                .await?
                .is_empty()
        );
        let nested = host.host().call_nested("acct/fixture", nest.id).await?;
        let [inner] = nested.as_slice() else {
            panic!("one nested call, got {}", nested.len());
        };
        assert_eq!((inner.id, inner.parent), (1, 0), "written: {written}");
        assert_eq!(
            (inner.service.as_str(), inner.tool.as_str()),
            ("inner", "echo")
        );
        assert_eq!(inner.outcome, Outcome::Done);
    }
    assert!(host.host().call_nested("acct/fixture", 999).await.is_err());
    Ok(())
}
