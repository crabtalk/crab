//! A service's vars: each deployed copy holds its own value of each, read
//! through `berm.var`, its default until one is set.

use anyhow::Result;
use berm_api::{
    Output, digest,
    host::{self, Deploy},
};
use berm_host::test::{self, TestHost};
use std::collections::BTreeMap;

mod common;

async fn greeted(host: &TestHost, service: &str) -> Result<Output> {
    Ok(host.call(service, "greet", b"{}").await?.result)
}

fn done(text: &str) -> Output {
    Output::Done(format!("{text:?}"))
}

#[tokio::test]
async fn a_var_reads_its_default_until_its_copy_sets_one() -> Result<()> {
    let host = common::deployed(&["acct/fixture", "other/fixture"]).await?;
    assert_eq!(greeted(&host, "acct/fixture").await?, done("hello"));
    let idl = host.host.idl("acct/fixture")?;
    assert_eq!(idl.vars["greeting"].description, "What greet answers");

    host.host
        .set_var("acct/fixture", "greeting", Some("hi".to_owned()))
        .await?;
    assert_eq!(greeted(&host, "acct/fixture").await?, done("hi"));
    assert_eq!(greeted(&host, "other/fixture").await?, done("hello"));
    let vars = host.host.vars("acct/fixture").await?;
    assert_eq!(vars.len(), 1);
    assert_eq!(vars[0].name, "greeting");
    assert_eq!(vars[0].default, "hello");
    assert_eq!(vars[0].value.as_deref(), Some("hi"));

    host.host.set_var("acct/fixture", "greeting", None).await?;
    assert_eq!(greeted(&host, "acct/fixture").await?, done("hello"));
    Ok(())
}

#[tokio::test]
async fn a_var_the_manifest_does_not_declare_is_refused() -> Result<()> {
    let host = common::deployed(&["acct/fixture"]).await?;
    let refused = host
        .host
        .set_var("acct/fixture", "missing", Some("x".to_owned()))
        .await
        .expect_err("an undeclared var is refused");
    assert_eq!(refused.status.as_u16(), 404);

    let too_long = "x".repeat(4097);
    let refused = host
        .host
        .set_var("acct/fixture", "greeting", Some(too_long))
        .await
        .expect_err("a value past 4096 bytes is refused");
    assert_eq!(refused.status.as_u16(), 400);

    let Output::Failed(failure) = host.call("acct/fixture", "undeclared", b"{}").await?.result
    else {
        panic!("reading an undeclared var fails the call");
    };
    assert!(
        failure.contains("missing is not declared in [vars]"),
        "{failure}"
    );
    Ok(())
}

#[tokio::test]
async fn a_deploy_sets_the_vars_it_names_and_keeps_the_rest() -> Result<()> {
    let host = TestHost::open(0).await?;
    let image = test::build("berm-fixture")?;
    host.deploy("acct/fixture", &image).await?;
    let redeploy = |vars: BTreeMap<String, String>| Deploy {
        reference: "test/acct/fixture".to_owned(),
        digest: digest::of(&image),
        force: true,
        crons: None,
        size: host::database_size(),
        stopped: false,
        check_secrets: false,
        project: None,
        exempt: Vec::new(),
        vars,
    };

    let named = BTreeMap::from([("greeting".to_owned(), "hey".to_owned())]);
    host.host
        .deploy("acct/fixture", redeploy(named), None)
        .await?;
    assert_eq!(greeted(&host, "acct/fixture").await?, done("hey"));

    host.host
        .deploy("acct/fixture", redeploy(BTreeMap::new()), None)
        .await?;
    assert_eq!(greeted(&host, "acct/fixture").await?, done("hey"));

    let undeclared = BTreeMap::from([("missing".to_owned(), "x".to_owned())]);
    let refused = host
        .host
        .deploy("acct/fixture", redeploy(undeclared), None)
        .await
        .err()
        .expect("a deploy naming an undeclared var is refused");
    assert_eq!(refused.status.as_u16(), 400);
    Ok(())
}
