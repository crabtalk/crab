//! `{secret:NAME}` in a fetch's headers and body, substituted by the host.

use anyhow::Result;
use berm_api::Output;
use berm_host::test::{self, TestHost};

const URL: &str = "http://127.0.0.1/token";

async fn deployed() -> Result<TestHost> {
    let host = TestHost::open(0).await?;
    host.deploy("fixture", &test::build("berm-fixture", "fixture")?)
        .await?;
    host.arrange("POST", URL, 200, "ok");
    Ok(host)
}

#[tokio::test]
async fn a_secret_in_the_body_is_substituted() -> Result<()> {
    let host = deployed().await?;
    host.grant("fixture", "fixture", "127.0.0.1", "s3cret");
    let ran = host
        .call(
            "fixture",
            "fetch",
            format!(r#"{{"url":"{URL}","header":"Authorization: Bearer {{secret:fixture}}","body":"a=1&secret={{secret:fixture}}&b=2"}}"#)
                .as_bytes(),
        )
        .await?;
    assert!(matches!(ran.result, Output::Done(_)), "{:?}", ran.result);
    let sent = host.fetched();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].body, b"a=1&secret=s3cret&b=2");
    assert_eq!(
        sent[0].headers,
        vec![("Authorization".to_owned(), "Bearer s3cret".to_owned())]
    );
    Ok(())
}

#[tokio::test]
async fn a_body_secret_nobody_granted_sends_nothing() -> Result<()> {
    let host = deployed().await?;
    let ran = host
        .call(
            "fixture",
            "fetch",
            format!(r#"{{"url":"{URL}","body":"secret={{secret:fixture}}"}}"#).as_bytes(),
        )
        .await?;
    let Output::Failed(failure) = ran.result else {
        panic!("an ungranted secret is refused");
    };
    assert!(failure.contains("no secret"), "{failure}");
    assert!(host.fetched().is_empty());
    Ok(())
}

#[tokio::test]
async fn a_body_secret_the_manifest_does_not_declare_sends_nothing() -> Result<()> {
    let host = deployed().await?;
    host.grant("fixture", "other", "127.0.0.1", "s3cret");
    let ran = host
        .call(
            "fixture",
            "fetch",
            format!(r#"{{"url":"{URL}","body":"secret={{secret:other}}"}}"#).as_bytes(),
        )
        .await?;
    let Output::Failed(failure) = ran.result else {
        panic!("an undeclared secret is refused");
    };
    assert!(failure.contains("does not declare"), "{failure}");
    assert!(host.fetched().is_empty());
    Ok(())
}
