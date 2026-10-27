//! A project's services deployed together: every one checked before any
//! switches, all switched or none.

use anyhow::Result;
use berm_api::host::{CallPage, Caller, DATABASE_SIZE};
use berm_host::test::{self, TestHost};

#[tokio::test]
async fn a_project_switches_together() -> Result<()> {
    let host = TestHost::open(0).await?;
    let image = test::build("berm-fixture")?;
    let deployed = host
        .deploy_set(
            "p",
            &[
                ("acct/one", &image, DATABASE_SIZE),
                ("acct/two", &image, DATABASE_SIZE),
            ],
        )
        .await?;
    assert_eq!(deployed.len(), 2);
    assert_eq!(
        host.host().runtime("acct/one").await?.project.as_deref(),
        Some("p")
    );
    assert!(host.host().get("acct/two").is_some());
    Ok(())
}

#[tokio::test]
async fn one_refused_refuses_the_project_and_none_switches() -> Result<()> {
    let host = TestHost::open(0).await?;
    let image = test::build("berm-fixture")?;
    let refused = host
        .deploy_set(
            "p",
            &[("acct/one", &image, DATABASE_SIZE), ("acct/two", &image, 0)],
        )
        .await
        .unwrap_err()
        .to_string();
    assert!(refused.contains("two:"), "{refused}");
    assert!(host.host().get("acct/one").is_none());
    assert!(host.host().get("acct/two").is_none());
    Ok(())
}

#[tokio::test]
async fn a_project_spans_one_scope() -> Result<()> {
    let host = TestHost::open(0).await?;
    let image = test::build("berm-fixture")?;
    let refused = host
        .deploy_set(
            "p",
            &[
                ("acct/one", &image, DATABASE_SIZE),
                ("other/two", &image, DATABASE_SIZE),
            ],
        )
        .await;
    assert!(refused.is_err());
    Ok(())
}

#[tokio::test]
async fn one_failing_to_switch_puts_those_switched_back() -> Result<()> {
    let host = TestHost::open(0).await?;
    host.deploy("acct/one", &test::build("berm-bare")?).await?;
    let failed = host
        .deploy_set(
            "p",
            &[
                ("acct/one", &test::build("berm-fixture")?, DATABASE_SIZE),
                ("acct/two", &test::build("berm-broken")?, DATABASE_SIZE),
            ],
        )
        .await
        .unwrap_err()
        .to_string();
    assert!(failed.contains("two failed to switch"), "{failed}");
    let one = host.host().get("acct/one").expect("one runs what it ran");
    assert_eq!(one.manifest().name, "berm-bare");
    assert!(host.host().get("acct/two").is_none());
    Ok(())
}

#[tokio::test]
async fn a_service_of_another_project_is_refused() -> Result<()> {
    let host = TestHost::open(0).await?;
    let image = test::build("berm-fixture")?;
    host.deploy_set("foo", &[("acct/hn", &image, DATABASE_SIZE)])
        .await?;
    let refused = host
        .deploy_set("bar", &[("acct/hn", &image, DATABASE_SIZE)])
        .await
        .unwrap_err()
        .to_string();
    assert!(refused.contains("hn is in project foo"), "{refused}");
    assert_eq!(
        host.host().runtime("acct/hn").await?.project.as_deref(),
        Some("foo")
    );
    Ok(())
}

#[tokio::test]
async fn a_project_names_its_services_and_their_calls() -> Result<()> {
    let host = TestHost::open(0).await?;
    let image = test::build("berm-fixture")?;
    host.deploy_set(
        "p",
        &[
            ("acct/one", &image, DATABASE_SIZE),
            ("acct/two", &image, DATABASE_SIZE),
        ],
    )
    .await?;
    host.deploy_set("q", &[("acct/three", &image, DATABASE_SIZE)])
        .await?;
    let caller = Caller {
        id: "acct".to_owned(),
        app: None,
        user: None,
        client: None,
        agent: None,
    };
    for service in ["acct/one", "acct/two", "acct/three"] {
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

    let listed = host.host().listings("acct").await?;
    let of = |name: &str| {
        listed
            .iter()
            .find(|listing| listing.program.name == name)
            .and_then(|listing| listing.project.clone())
    };
    assert_eq!(of("one").as_deref(), Some("p"));
    assert_eq!(of("three").as_deref(), Some("q"));
    // A registry reference another account can deploy; the test host's images
    // are all named by one.
    let reference = listed
        .iter()
        .find(|listing| listing.program.name == "one")
        .and_then(|listing| listing.reference.clone());
    assert_eq!(reference.as_deref(), Some("test/acct/one"));

    let page = CallPage {
        service: None,
        project: Some("p".to_owned()),
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
        let (calls, _) = host.host().scope_calls("acct", page.clone()).await?;
        let mut services: Vec<&str> = calls.iter().map(|call| call.service.as_str()).collect();
        services.sort();
        assert_eq!(services, ["one", "two"], "written: {written}");
    }
    Ok(())
}
