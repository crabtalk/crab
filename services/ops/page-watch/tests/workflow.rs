use anyhow::Result;
use berm::syscall::http::Response;
use berm_api::{
    host::Caller,
    workflow::run::{Run, State, Status, Trigger},
};
use berm_build::fixture;
use berm_host::{test::TestHost, workflow::StorageGrant};
use serde_json::{Value, json};
use std::time::Duration;

fn page(host: &TestHost, url: &str, status: u16, text: &str) {
    host.fetch.respond(
        "GET",
        url,
        Response {
            status,
            headers: vec![("content-type".into(), "text/html".into())],
            body: format!("<html><body><main><p>{text}</p></main></body></html>").into_bytes(),
        },
        Duration::ZERO,
    );
}

async fn run(host: &TestHost, input: Value) -> Result<Run> {
    let image = fixture::image("crab-page-watch")?;
    let run = host.host.prepare_workflow(
        "watch",
        "page_watch",
        input,
        Trigger::Manual {
            caller: Caller {
                id: "operator".into(),
                key: None,
                app: None,
                user: None,
                client: None,
                agent: None,
            },
        },
    )?;
    host.host
        .workflows
        .insert(
            run.clone(),
            image,
            StorageGrant {
                scope: "".into(),
                bytes: u64::MAX,
            },
        )
        .await?;
    host.host.execute_workflow(&run.id).await
}

#[tokio::test]
#[ignore = "requires the prebuilt BERM_TEST_IMAGE_CRAB_PAGE_WATCH image"]
async fn changed_and_unchanged_pages_take_the_declared_branches() -> Result<()> {
    let host = TestHost::open(100).await?;
    host.deploy("watch", &fixture::image("crab-page-watch")?)
        .await?;
    page(&host, "https://example.com/a", 200, "First version");
    let input = json!({"urls":["https://example.com/a"]});
    let first = run(&host, input.clone()).await?;
    assert_eq!(first.status, Status::Succeeded, "{:?}", first.error);
    assert_eq!(
        first.result.unwrap(),
        json!({"checked":1,"changed":["https://example.com/a"],"saved":true})
    );
    let same = run(&host, input.clone()).await?;
    assert_eq!(same.status, Status::Succeeded, "{:?}", same.error);
    assert!(matches!(same.checkpoints["save"].state, State::Skipped));
    assert_eq!(
        same.result.unwrap(),
        json!({"checked":1,"changed":[],"saved":false})
    );
    page(&host, "https://example.com/a", 200, "Second version");
    let changed = run(&host, input).await?;
    assert_eq!(changed.status, Status::Succeeded, "{:?}", changed.error);
    assert_eq!(changed.result.unwrap()["saved"], true);
    Ok(())
}

#[tokio::test]
#[ignore = "requires the prebuilt BERM_TEST_IMAGE_CRAB_PAGE_WATCH image"]
async fn continuation_reuses_successful_reads_after_a_known_failure() -> Result<()> {
    let host = TestHost::open(100).await?;
    host.deploy("watch", &fixture::image("crab-page-watch")?)
        .await?;
    page(&host, "https://example.com/a", 200, "First page");
    page(&host, "https://example.com/b", 503, "Unavailable");
    let failed = run(
        &host,
        json!({"urls":["https://example.com/a","https://example.com/b"]}),
    )
    .await?;
    assert_eq!(failed.status, Status::Failed, "{:?}", failed.error);
    assert!(matches!(
        failed.checkpoints["pages"].items[0].state,
        State::Succeeded { .. }
    ));
    page(&host, "https://example.com/b", 200, "Recovered page");
    let complete = host.host.continue_workflow(&failed.id).await?;
    assert_eq!(complete.status, Status::Succeeded, "{:?}", complete.error);
    assert_eq!(complete.attempts.len(), 2);
    assert_eq!(complete.result.unwrap()["checked"], 2);
    assert_eq!(
        host.fetched()
            .iter()
            .filter(|request| request.url == "https://example.com/a")
            .count(),
        1
    );
    Ok(())
}

#[tokio::test]
#[ignore = "requires the prebuilt BERM_TEST_IMAGE_CRAB_PAGE_WATCH image"]
async fn oversized_batches_fail_before_the_first_http_call() -> Result<()> {
    let host = TestHost::open(100).await?;
    host.deploy("watch", &fixture::image("crab-page-watch")?)
        .await?;
    let failed = run(&host, json!({"urls":["https://example.com/a","https://example.com/b","https://example.com/c","https://example.com/d"]})).await?;
    assert_eq!(failed.status, Status::Failed);
    assert!(host.fetched().is_empty());
    Ok(())
}
