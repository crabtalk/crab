use anyhow::{Context, Result};
use berm::{Berm, Calls, Config, Engine};
use berm_api::Output;
use berm_build::fixture;
use std::sync::Arc;

#[tokio::test]
async fn a_nested_call_returns_its_payload_and_a_missing_target_is_refused() -> Result<()> {
    let image = fixture::image("berm-fixture")?;
    let engine = Engine::new(&Config::default())?;
    let berm = Berm::new(&engine, Calls::by_name(Calls::DEFAULT_DEPTH), vec![]);
    berm.deploy("inner", &image)?;
    berm.deploy("outer", &image)?;

    let args = br#"{"query":"hi"}"#;
    let called = berm.call(None, "outer", "nest", args.to_vec()).await?;
    assert_eq!(called.result, Output::Done(r#"{"query":"hi"}"#.into()));

    assert!(berm.remove("inner"));
    let called = berm.call(None, "outer", "nest", args.to_vec()).await?;
    let Output::Failed(failure) = called.result else {
        panic!("a missing nested target is refused");
    };
    assert!(failure.starts_with("refused: "), "{failure}");
    Ok(())
}

#[tokio::test]
async fn recursive_calls_stop_at_the_configured_depth() -> Result<()> {
    let image = fixture::image("berm-fixture")?;
    let engine = Engine::new(&Config::default())?;
    for depth in [0, 2] {
        let berm = Berm::new(&engine, Calls::by_name(depth), vec![]);
        berm.deploy("inner", &image)?;
        let called = berm
            .call(None, "inner", "recurse", br#"{"depth":0}"#.to_vec())
            .await?;
        assert_eq!(called.result, Output::Done((depth + 1).to_string()));
    }
    Ok(())
}

#[tokio::test]
async fn a_resolver_keeps_nested_calls_inside_the_callers_scope() -> Result<()> {
    let image = fixture::image("berm-fixture")?;
    let engine = Engine::new(&Config::default())?;
    let berm = Berm::new(
        &engine,
        Calls {
            resolve: Arc::new(|at, target, _| {
                let (owner, _) = at
                    .service
                    .split_once('/')
                    .context("deployed without an owner")?;
                Ok(format!("{owner}/{target}"))
            }),
            named: Arc::new(|service| {
                service
                    .split_once('/')
                    .map_or(service, |(_, name)| name)
                    .to_owned()
            }),
            ..Calls::by_name(Calls::DEFAULT_DEPTH)
        },
        vec![],
    );
    berm.deploy("alice/outer", &image)?;
    berm.deploy("alice/inner", &image)?;
    berm.deploy("bob/outer", &image)?;

    let args = br#"{"query":"hi"}"#;
    let called = berm
        .call(None, "alice/outer", "nest", args.to_vec())
        .await?;
    assert_eq!(called.result, Output::Done(r#"{"query":"hi"}"#.into()));
    let called = berm.call(None, "bob/outer", "nest", args.to_vec()).await?;
    let Output::Failed(failure) = called.result else {
        panic!("Bob cannot reach Alice's inner service");
    };
    assert!(failure.starts_with("refused: "), "{failure}");
    assert!(failure.contains("bob/inner"), "{failure}");
    Ok(())
}
