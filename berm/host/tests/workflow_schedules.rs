use anyhow::Result;
use axum::{body::to_bytes, response::IntoResponse};
use berm_api::{
    Image,
    workflow::{
        run::{Status, Trigger},
        schedule::Occurrence,
    },
};
use berm_build::fixture;
use berm_host::{Denied, test::TestHost};
use serde_json::{Value, json};
use std::time::Duration;

fn configured(original: &[u8], input: Value, cron: Option<&str>) -> Vec<u8> {
    let mut record: Value = serde_json::from_slice(
        Image(original)
            .section(berm_abi::SERVICE_SECTION)
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    record["workflows"]["workflow_flow"]["schedule"] = cron
        .map(|cron| json!({"cron":cron,"input":input}))
        .unwrap_or(Value::Null);
    Image(original)
        .replace_section(
            berm_abi::SERVICE_SECTION,
            &serde_json::to_vec(&record).unwrap(),
        )
        .unwrap()
}
async fn wait(host: &TestHost, id: &str) -> Result<()> {
    for _ in 0..500 {
        let run = host.host.workflows.get(id).await?.unwrap();
        if run.status == Status::Succeeded {
            assert_eq!(run.attempts.len(), 1);
            return Ok(());
        }
        assert!(
            matches!(run.status, Status::Pending | Status::Running),
            "{:?}",
            run.error
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("scheduled workflow did not finish")
}

#[tokio::test]
async fn utc_occurrences_copy_inputs_and_duplicate_delivery_never_starts_another_attempt()
-> Result<()> {
    let host = TestHost::open(1).await?;
    let image = configured(
        &fixture::image("berm-fixture")?,
        json!({"count":2,"take":true}),
        Some("* * * * *"),
    );
    host.deploy("owner/flow", &image).await?;
    host.set_now(60_000);
    let occurrence = host.host.due_workflows().await?.pop().unwrap();
    assert_eq!(occurrence.at, 60_000);
    let id = occurrence.id();
    assert!(host.host.due_workflows().await?.is_empty());
    let (a, b) = tokio::join!(
        host.host.fire_scheduled_workflow(occurrence.clone()),
        host.host.fire_scheduled_workflow(occurrence.clone())
    );
    assert_eq!(a?, id);
    assert_eq!(b?, id);
    wait(&host, &id).await?;
    let run = host.host.workflows.get(&id).await?.unwrap();
    assert_eq!(run.input, occurrence.input);
    assert!(matches!(run.trigger,Trigger::Schedule {key,at} if key==id && at==60_000));
    let mut upgraded: Value =
        serde_json::from_slice(Image(&image).section(berm_abi::SERVICE_SECTION)?.unwrap())?;
    upgraded["version"] = json!("0.2.0");
    let upgraded = Image(&image)
        .replace_section(berm_abi::SERVICE_SECTION, &serde_json::to_vec(&upgraded)?)?;
    host.deploy("owner/flow", &upgraded).await?;
    assert_eq!(host.host.fire_scheduled_workflow(occurrence).await?, id);
    let retained = host.host.workflows.get(&id).await?.unwrap();
    assert_eq!(retained.source, run.source);
    assert_eq!(retained.attempts.len(), 1);
    host.set_now(120_000);
    let next = host.host.due_workflows().await?.pop().unwrap();
    assert_ne!(next.id(), id);
    Ok(())
}

#[tokio::test]
async fn restart_stopped_services_and_clock_gaps_do_not_backfill_occurrences() -> Result<()> {
    let mut host = TestHost::open(1).await?;
    let image = configured(
        &fixture::image("berm-fixture")?,
        json!({"count":0,"take":false}),
        Some("* * * * *"),
    );
    host.deploy("owner/flow", &image).await?;
    host.set_now(180_001);
    host.reopen().await?;
    host.host.reset_workflow_schedules().await?;
    assert!(host.host.due_workflows().await?.is_empty());
    host.set_now(240_000);
    let prior = host.host.due_workflows().await?.pop().unwrap();
    host.host.stop("owner/flow").await?;
    let denied =
        Denied::from(host.host.fire_scheduled_workflow(prior).await.unwrap_err()).into_response();
    assert_eq!(denied.status(), 503);
    host.set_now(300_000);
    assert!(host.host.due_workflows().await?.is_empty());
    host.host.start("owner/flow").await?;
    assert!(host.host.due_workflows().await?.is_empty());
    host.set_now(360_000);
    assert_eq!(host.host.due_workflows().await?.len(), 1);
    host.set_now(540_001);
    assert!(host.host.due_workflows().await?.is_empty());
    Ok(())
}

#[tokio::test]
async fn manifest_schedule_changes_fence_old_inputs_and_removal_disarms_future_runs() -> Result<()>
{
    let host = TestHost::open(1).await?;
    let original = fixture::image("berm-fixture")?;
    let image = configured(
        &original,
        json!({"count":1,"take":false}),
        Some("* * * * *"),
    );
    host.deploy("owner/flow", &image).await?;
    host.set_now(60_000);
    let old = host.host.due_workflows().await?.pop().unwrap();
    let changed = configured(&original, json!({"count":3,"take":true}), Some("* * * * *"));
    host.deploy("owner/flow", &changed).await?;
    let denied = Denied::from(
        host.host
            .fire_scheduled_workflow(old.clone())
            .await
            .unwrap_err(),
    )
    .into_response();
    assert_eq!(denied.status(), 409);
    let body: Value = serde_json::from_slice(&to_bytes(denied.into_body(), usize::MAX).await?)?;
    assert_eq!(body["explain"], Occurrence::STALE);
    assert!(host.host.workflows.get(&old.id()).await?.is_none());
    host.set_now(120_000);
    let next = host.host.due_workflows().await?.pop().unwrap();
    assert_eq!(next.input, json!({"count":3,"take":true}));
    let removed = configured(&original, Value::Null, None);
    host.deploy("owner/flow", &removed).await?;
    host.set_now(180_000);
    assert!(host.host.due_workflows().await?.is_empty());
    host.host.remove("owner/flow").await?;
    assert!(host.host.due_workflows().await?.is_empty());
    Ok(())
}

#[tokio::test]
async fn invalid_manifest_schedule_input_keeps_shared_explanation_before_install() -> Result<()> {
    let host = TestHost::open(1).await?;
    let image = configured(
        &fixture::image("berm-fixture")?,
        json!("invalid"),
        Some("* * * * *"),
    );
    let denied =
        Denied::from(host.deploy("owner/flow", &image).await.err().unwrap()).into_response();
    assert_eq!(denied.status(), 400);
    let body: Value = serde_json::from_slice(&to_bytes(denied.into_body(), usize::MAX).await?)?;
    assert_eq!(body["explain"], "E0074");
    assert!(host.host.due_workflows().await?.is_empty());
    Ok(())
}
