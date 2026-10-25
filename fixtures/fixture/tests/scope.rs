//! A scope's calls: every service's runs in it, newest first, each naming its
//! service, and none of another scope's.

use anyhow::Result;
use berm_api::host::{CallPage, Caller};
use berm_host::test::{self, TestHost};

fn page(service: Option<&str>, before: Option<u64>) -> CallPage {
    CallPage {
        service: service.map(str::to_owned),
        tool: None,
        event: None,
        q: None,
        outcome: None,
        since: None,
        until: None,
        before,
        limit: 10,
    }
}

#[tokio::test]
async fn a_scope_lists_each_service_s_calls_newest_first() -> Result<()> {
    let host = TestHost::open(0).await?;
    let image = test::build("berm-fixture")?;
    for service in ["acct/fixture", "acct/inner", "other/fixture"] {
        host.deploy(service, &image).await?;
    }
    let caller = Caller {
        id: "acct".to_owned(),
        app: None,
        user: None,
        client: None,
        agent: None,
    };
    for service in ["acct/fixture", "acct/inner", "other/fixture"] {
        host.host()
            .call(
                service,
                Some(&caller),
                "echo",
                br#"{"query":"hi"}"#.to_vec(),
                false,
            )
            .await?;
    }

    for written in [false, true] {
        if written {
            host.flush().await?;
        }
        let (calls, _) = host.host().scope_calls("acct", page(None, None)).await?;
        let services: Vec<&str> = calls.iter().map(|ran| ran.service.as_str()).collect();
        assert_eq!(services, ["inner", "fixture"], "written: {written}");
        assert!(calls[0].call.id > calls[1].call.id);
    }

    let (calls, _) = host
        .host()
        .scope_calls("acct", page(Some("fixture"), None))
        .await?;
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].service, "fixture");

    let below = calls[0].call.id;
    let (calls, _) = host
        .host()
        .scope_calls("acct", page(None, Some(below)))
        .await?;
    assert!(calls.is_empty());
    Ok(())
}

#[tokio::test]
async fn a_search_finds_the_calls_whose_logs_hold_its_text() -> Result<()> {
    let host = TestHost::open(0).await?;
    host.deploy("acct/fixture", &test::build("berm-fixture")?)
        .await?;
    let caller = Caller {
        id: "acct".to_owned(),
        app: None,
        user: None,
        client: None,
        agent: None,
    };
    for word in ["Order-42", "other"] {
        let args = format!(r#"{{"word":"{word}"}}"#);
        host.host()
            .call(
                "acct/fixture",
                Some(&caller),
                "relay",
                args.into_bytes(),
                false,
            )
            .await?;
    }

    for written in [false, true] {
        if written {
            host.flush().await?;
        }
        let found = |q: &str| {
            let host = host.host().clone();
            let page = CallPage {
                q: Some(q.to_owned()),
                ..page(None, None)
            };
            async move {
                host.scope_calls("acct", page)
                    .await
                    .map(|(calls, _)| calls.len())
            }
        };
        assert_eq!(found("order-42").await?, 1, "written: {written}");
        assert_eq!(found("ORDER").await?, 1, "written: {written}");
        assert_eq!(
            found("relay").await?,
            2,
            "a topic holds it; written: {written}"
        );
        assert_eq!(found("missing").await?, 0, "written: {written}");
    }
    Ok(())
}
