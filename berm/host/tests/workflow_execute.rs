use anyhow::Result;
use axum::{body::to_bytes, response::IntoResponse};
use berm::{Calls, Enter, syscall::http::Response, workflow as spend};
use berm_api::{
    Image, Manifest, Output,
    host::{Caller, Link, User},
    workflow::{
        graph::{Kind, Target},
        run::{Failure, Run, State, Status, Trigger},
        spend::Context as SpendContext,
    },
};
use berm_build::fixture;
use berm_host::{Config, test::TestHost, workflow::StorageGrant};
use serde_json::{Value, json};
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

fn trigger() -> Trigger {
    Trigger::Manual {
        caller: Caller {
            id: "caller".into(),
            key: Some("key-1".into()),
            app: None,
            user: Some(User {
                issuer: "verified".into(),
                subject: "reader".into(),
            }),
            client: None,
            agent: None,
        },
    }
}

async fn admit(host: &TestHost, workflow: &str, input: Value) -> Result<Run> {
    let image = fixture::image("berm-fixture")?;
    host.deploy("owner/flow", &image).await?;
    let run = host
        .host
        .prepare_workflow("owner/flow", workflow, input, trigger())?;
    host.host
        .workflows
        .insert(
            run.clone(),
            image,
            StorageGrant {
                scope: "owner".into(),
                bytes: u64::MAX,
            },
        )
        .await?;
    Ok(run)
}

#[tokio::test]
async fn real_typed_forks_branches_optional_calls_and_ordered_items_finish() -> Result<()> {
    for take in [false, true] {
        let mut host = TestHost::open(100).await?;
        let run = admit(&host, "workflow_flow", json!({"count":3,"take":take})).await?;
        let complete = host.host.execute_workflow(&run.id).await?;
        assert_eq!(complete.status, Status::Succeeded, "{:?}", complete.error);
        let result = complete.result.unwrap();
        assert_eq!(result["left"], 33);
        assert_eq!(result["right"], 1026);
        assert_eq!(result["values"], json!([0, 1, 2]));
        assert_eq!(
            result["identity"],
            "service owner/flow user verified reader key key-1"
        );
        assert_eq!(
            result["optional"],
            if take { json!(99) } else { Value::Null }
        );
        assert_eq!(result["maybe"], if take { json!(88) } else { Value::Null });
        assert_eq!(
            complete.checkpoints["items"]
                .items
                .iter()
                .map(|item| item.index)
                .collect::<Vec<_>>(),
            [0, 1, 2]
        );
        assert!(complete.attempts[0].meter.calls >= 10);
        let stored: Value =
            serde_json::from_slice(&host.state("owner/flow", "workflow_items").await?.unwrap())?;
        assert_eq!(
            stored,
            if take {
                json!([99, 88, 0, 1, 2])
            } else {
                json!([0, 1, 2])
            }
        );
        // The workflow's private tool remains unreachable from the outside.
        assert!(
            host.host
                .call(
                    "owner/flow",
                    None,
                    "workflow_item",
                    br#"{"index":8}"#.to_vec(),
                    false
                )
                .await
                .is_err()
        );
        host.reopen().await?;
        assert_eq!(
            host.host.workflows.get(&run.id).await?.unwrap().status,
            Status::Succeeded
        );
    }
    Ok(())
}

#[tokio::test]
async fn empty_each_succeeds_and_overflow_dispatches_none_of_its_items() -> Result<()> {
    for count in [0, 4] {
        let host = TestHost::open(100).await?;
        let run = admit(&host, "workflow_flow", json!({"count":count,"take":false})).await?;
        let result = host.host.execute_workflow(&run.id).await?;
        assert!(host.state("owner/flow", "workflow_items").await?.is_none());
        assert!(result.checkpoints["items"].items.is_empty());
        if count == 0 {
            assert_eq!(result.status, Status::Succeeded, "{:?}", result.error);
            assert_eq!(result.result.unwrap()["values"], json!([]));
        } else {
            assert_eq!(result.status, Status::Failed);
            assert!(matches!(
                result.checkpoints["items"].state,
                State::Failed { .. }
            ));
            assert!(matches!(result.checkpoints["done"].state, State::Pending));
        }
    }
    Ok(())
}

#[tokio::test]
async fn failed_item_stops_later_items_and_is_never_optional_absence() -> Result<()> {
    let host = TestHost::open(100).await?;
    let run = admit(
        &host,
        "workflow_flow",
        json!({"count":3,"take":false,"fail_at":1}),
    )
    .await?;
    let result = host.host.execute_workflow(&run.id).await?;
    assert_eq!(result.status, Status::Failed, "{:?}", result.error);
    let items = &result.checkpoints["items"].items;
    assert!(matches!(items[0].state, State::Succeeded { .. }));
    assert!(matches!(items[1].state, State::Failed { .. }));
    assert!(matches!(items[2].state, State::Pending));
    assert_eq!(result.error.as_ref().unwrap().body["error"], "item failed");
    assert_eq!(
        host.state("owner/flow", "workflow_items").await?,
        Some(b"[0]".to_vec())
    );
    assert!(matches!(result.checkpoints["done"].state, State::Pending));
    let host = TestHost::open(100).await?;
    let run = admit(
        &host,
        "workflow_flow",
        json!({"count":0,"take":true,"fail_at":99}),
    )
    .await?;
    let result = host.host.execute_workflow(&run.id).await?;
    assert_eq!(result.status, Status::Failed);
    assert!(matches!(
        result.checkpoints["optional"].state,
        State::Failed { .. }
    ));
    assert!(matches!(result.checkpoints["done"].state, State::Pending));
    Ok(())
}

#[tokio::test]
async fn changed_dependency_is_refused_before_tool_dispatch() -> Result<()> {
    let host = TestHost::open(100).await?;
    let image = fixture::image("berm-fixture")?;
    host.deploy("owner/fixture", &image).await?;
    let run = admit(&host, "workflow_external", json!(0)).await?;
    let changed = Image(&image).with_section("workflow-test-change", b"changed")?;
    host.deploy("owner/fixture", &changed).await?;
    let result = host.host.execute_workflow(&run.id).await?;
    assert_eq!(result.status, Status::Failed);
    assert!(result.checkpoints["ask"].items[0].dispatches.is_empty());
    assert_eq!(result.error.unwrap().body["explain"], "E0060");
    let link = Link {
        workflow: None,
        operation: None,
        accepted: Some(run.accepted.bindings["ask"].clone()),
        caller: Some("owner/flow".into()),
        from: "owner/flow".into(),
        key: None,
        user: None,
        client: None,
        client_ip: None,
        depth: 1,
        query: false,
        timeout: 1000,
        stack: vec!["owner/flow".into(), "owner/fixture".into()],
    };
    let response = host
        .host
        .link("owner/fixture", "whoami", b"{}".to_vec(), link)
        .await
        .unwrap_err()
        .into_response();
    assert_eq!(response.status(), 409);
    let body: Value = serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await?)?;
    assert_eq!(body["explain"], "E0060");

    Ok(())
}

#[tokio::test]
async fn original_image_and_private_targets_survive_a_workflow_service_upgrade() -> Result<()> {
    let host = TestHost::open(100).await?;
    let run = admit(&host, "workflow_flow", json!({"count":2,"take":false})).await?;
    let old = fixture::image("berm-fixture")?;
    let changed = Image(&old).with_section("workflow-test-change", b"new service")?;
    host.deploy("owner/flow", &changed).await?;
    let result = host.host.execute_workflow(&run.id).await?;
    assert_eq!(result.status, Status::Succeeded, "{:?}", result.error);
    assert_eq!(result.source.image, run.source.image);
    assert_ne!(
        berm_api::digest::prefixed(&host.host.get("owner/flow").unwrap().digest),
        run.source.image
    );
    Ok(())
}

#[tokio::test]
async fn tool_exhaustion_is_unknown_and_one_attempt_deadline_bounds_all_steps() -> Result<()> {
    let host = TestHost::open(100).await?;
    let run = admit(&host, "workflow_example", json!(u64::MAX)).await?;
    let result = host.host.execute_workflow(&run.id).await?;
    assert_eq!(result.status, Status::Indeterminate);
    assert!(matches!(
        result.checkpoints["spin"].items[0].state,
        State::Unknown { .. }
    ));
    assert!(result.attempts[0].meter.cpu > 0);
    let host = TestHost::open(100).await?;
    let image = fixture::image("berm-fixture")?;
    host.deploy("owner/flow", &image).await?;
    let mut run = host.host.prepare_workflow(
        "owner/flow",
        "workflow_flow",
        json!({"count":3,"take":true}),
        trigger(),
    )?;
    run.accepted.deadline_ms = 1;
    host.host
        .workflows
        .insert(
            run.clone(),
            image,
            StorageGrant {
                scope: "owner".into(),
                bytes: u64::MAX,
            },
        )
        .await?;
    let began = Instant::now();
    let result = host.host.execute_workflow(&run.id).await?;
    assert!(matches!(
        result.status,
        Status::Failed | Status::Indeterminate
    ));
    assert!(began.elapsed().as_secs() < 3);
    assert!(
        result.error.unwrap().body["error"]
            .as_str()
            .unwrap()
            .contains("deadline")
    );
    Ok(())
}

#[path = "support/workflow.rs"]
mod ledger_fixture;

#[tokio::test]
async fn host_enforces_each_bounds_on_untrusted_control_outputs() -> Result<()> {
    let host = TestHost::open(100).await?;
    let image = ledger_fixture::image(&[json!(0), json!(1), json!(2), json!(3)]);
    host.deploy("owner/identity", &image).await?;
    let mut run = ledger_fixture::run("overflow", &image);
    run.accepted.control_budget = host.host.get("owner/identity").unwrap().controls["identity"];
    host.host
        .workflows
        .insert(
            run.clone(),
            image,
            StorageGrant {
                scope: "owner".into(),
                bytes: u64::MAX,
            },
        )
        .await?;
    let result = host.host.execute_workflow(&run.id).await?;
    assert_eq!(result.status, Status::Failed);
    assert!(result.checkpoints["send"].items.is_empty());
    assert_eq!(result.attempts[0].meter.calls, 1);
    assert!(
        result.error.unwrap().body["error"]
            .as_str()
            .unwrap()
            .contains("call bound")
    );
    Ok(())
}

#[tokio::test]
async fn memory_and_pause_refusals_keep_shared_explanations_in_run_errors() -> Result<()> {
    for paused in [false, true] {
        let host = TestHost::open(100).await?;
        let run = admit(&host, "workflow_flow", json!({"count":0,"take":false})).await?;
        if paused {
            host.host.stop("owner/flow").await?;
        } else {
            host.host.limit([("owner".to_owned(), 1)].into());
        }
        let result = host.host.execute_workflow(&run.id).await?;
        assert_eq!(result.status, Status::Failed);
        let failure = result.error.unwrap();
        assert_eq!(failure.status, if paused { 503 } else { 429 });
        assert_eq!(
            failure.body["explain"],
            if paused { "E0018" } else { "E0019" }
        );
        assert!(result.checkpoints.values().all(|checkpoint| {
            checkpoint
                .items
                .iter()
                .all(|item| item.dispatches.is_empty())
        }));
    }
    Ok(())
}

#[tokio::test]
async fn an_in_flight_failure_keeps_its_error_and_meter_after_the_dispatch_lease() -> Result<()> {
    let host = TestHost::open(100).await?;
    let run = admit(&host, "workflow_flaky", json!("http://127.0.0.1/late")).await?;
    let release = host.fetch.respond_blocked(
        "GET",
        "http://127.0.0.1/late",
        Response {
            status: 200,
            headers: vec![],
            body: b"recorded late".to_vec(),
        },
    );
    let executing = host.host.clone();
    let id = run.id.clone();
    let task = tokio::spawn(async move { executing.execute_workflow(&id).await });
    tokio::time::timeout(Duration::from_secs(5), async {
        while host.fetched().is_empty() {
            tokio::task::yield_now().await;
        }
    })
    .await?;
    host.set_now(run.created_at + run.accepted.deadline_ms + 1);
    release.notify_one();
    let result = task.await??;
    assert_eq!(result.status, Status::Failed);
    assert!(
        result.error.unwrap().body["error"]
            .as_str()
            .unwrap()
            .contains("recorded late")
    );
    assert!(result.attempts[0].meter.cpu > 0);
    assert_eq!(
        result.checkpoints["call"].items[0].dispatches[0].finished_at,
        Some(host.now())
    );
    Ok(())
}

#[path = "../../api/tests/support/workflow.rs"]
mod workflow_image;

#[tokio::test]
async fn quote_reuses_the_service_callers_explanation_for_account_only_tools() -> Result<()> {
    let host = TestHost::open(100).await?;
    let fixture = fixture::image("berm-fixture")?;
    host.deploy("owner/fixture", &fixture).await?;
    let manifest = Manifest::try_from(fixture.as_slice())?;
    let mut entry = manifest
        .workflows
        .iter()
        .find(|spec| spec.entry.name == "workflow_external")
        .unwrap()
        .entry
        .clone();
    entry.name = "identity".into();
    for node in &mut entry.graph.nodes {
        node.export = format!("berm_workflow_identity_{}", node.id);
    }
    let Kind::Step { call, .. } = &mut entry.graph.nodes[0].kind else {
        panic!("no step")
    };
    call.target = Target::Service {
        service: "fixture".into(),
        tool: "fromaccounts".into(),
        idempotency_ms: None,
    };
    let wasm = wat::parse_str(
        "(module (memory (export \"memory\") 1) (func (export \"berm_workflow_identity_ask\")) (func (export \"berm_workflow_identity_done\")))",
    )?;
    let image = workflow_image::with_record(
        &wasm,
        json!({"workflow":entry,"abi_version":berm::abi::ABI_VERSION,"crate":"workflow-only"}),
        json!({"max_cost":1000,"checkpoint_bytes":4096}),
    );
    host.deploy("owner/flow", &image).await?;
    let error = host
        .host
        .workflow_bindings("owner/flow", "identity")
        .unwrap_err()
        .downcast::<berm_host::Denied>()?;
    assert_eq!(error.status, 403);
    assert_eq!(error.explain, Some("E0049"));
    Ok(())
}

#[tokio::test]
async fn a_failed_terminal_write_exposes_neither_output_nor_success() -> Result<()> {
    let host = TestHost::open(100).await?;
    let run = admit(&host, "workflow_example", json!(3)).await?;
    host.host.workflows.store.run(|conn|conn.execute_batch("CREATE TRIGGER refuse_completion BEFORE UPDATE ON runs WHEN json_extract(CAST(NEW.body AS TEXT),'$.status')='succeeded' BEGIN SELECT RAISE(ABORT,'terminal write refused'); END;")).await?;
    assert!(host.host.execute_workflow(&run.id).await.is_err());
    let retained = host.host.workflows.get(&run.id).await?.unwrap();
    assert_eq!(retained.status, Status::Running);
    assert!(retained.result.is_none());
    assert!(matches!(retained.checkpoints["done"].state, State::Pending));
    assert!(matches!(
        retained.checkpoints["spin"].state,
        State::Succeeded { .. }
    ));
    Ok(())
}

#[tokio::test]
async fn workflow_spend_follows_nested_calls_and_refuses_owner_work_after_the_attempt() -> Result<()>
{
    let seen = Arc::new(Mutex::new(Vec::new()));
    let observed = seen.clone();
    let unheld = Calls::by_name(4).enter;
    let enter: Enter = Arc::new(move |service, at| {
        observed
            .lock()
            .unwrap()
            .push((at.service.to_string(), at.workflow.clone()));
        unheld(service, at)
    });
    let host = TestHost::open_with_scope(100, Config::default(), Some(enter)).await?;
    let image = fixture::image("berm-fixture")?;
    host.deploy("owner/source", &image).await?;
    host.deploy("owner/fixture", &image).await?;
    let context = SpendContext {
        run: "run".into(),
        attempt: "attempt".into(),
        owner: "owner".into(),
        deadline_at: 100_000,
        authorization: "private-grant".into(),
    };
    let args =
        serde_json::to_vec(&json!({"service":"fixture","tool":"whoami","args":"{}","delay":null}))?;
    let called = spend::scope(context.clone(), host.call("owner/source", "invoke", &args)).await?;
    assert!(matches!(called.result, Output::Done(_)));
    let seen = seen.lock().unwrap().clone();
    assert_eq!(seen.len(), 2);
    assert!(
        seen.iter()
            .all(|(_, found)| *found == Some(context.clone()))
    );
    let args =
        serde_json::to_vec(&json!({"service":"fixture","tool":"whoami","args":"{}","delay":1}))?;
    let called = spend::scope(context.clone(), host.call("owner/source", "invoke", &args)).await?;
    let Output::Failed(message) = called.result else {
        anyhow::bail!("deferred call was accepted");
    };
    assert!(message.contains("E0063"), "{message}");
    assert!(host.wakes().await?.is_empty());
    let mut expired = context;
    expired.deadline_at = 100;
    let service = host.host.get("owner/source").unwrap();
    let error = spend::scope(expired, service.call(None, "whoami", b"{}".to_vec()))
        .await
        .unwrap_err();
    let failure = error.downcast_ref::<Failure>().unwrap();
    assert_eq!(failure.status, 409);
    assert_eq!(failure.body["explain"], "E0062");

    Ok(())
}

#[tokio::test]
async fn native_offer_keeps_internal_bindings_and_nested_each_results() -> Result<()> {
    let host = TestHost::open(100).await?;
    let image = fixture::image("berm-fixture")?;
    host.deploy("owner/fixture", &image).await?;
    let run = admit(
        &host,
        "workflow_nested",
        json!({"service":"fixture","tool":"whoami","args":"{}"}),
    )
    .await?;
    let offer = host.host.workflow_offer("owner/flow", "workflow_nested")?;
    assert_eq!(offer.source, run.source);
    assert_eq!(offer.local_bindings, run.accepted.bindings);
    let run = host.host.execute_workflow(&run.id).await?;
    assert_eq!(run.status, Status::Succeeded, "{:?}", run.error);
    assert_eq!(run.checkpoints["calls"].items.len(), 2);
    Ok(())
}
