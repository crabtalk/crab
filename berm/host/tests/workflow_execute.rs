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
    assert!(
        result.checkpoints["ask"]
            .items
            .iter()
            .all(|item| item.dispatches.is_empty())
    );
    assert_eq!(result.error.unwrap().body["explain"], "E0060");
    let link = Link {
        retained_until: None,
        replay: None,
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
    let fixture = fixture::image("berm-fixture")?;
    host.deploy("owner/fixture", &fixture).await?;
    let binding = host.host.workflow_binding("owner/fixture", "echo")?;
    let bytes = ledger_fixture::image(&[json!(0), json!(1), json!(2), json!(3)]);
    let mut record: Value =
        serde_json::from_slice(Image(&bytes).section(berm::abi::ABI_SECTION)?.unwrap())?;
    record["workflow"]["graph"]["nodes"][0]["call"] = json!({"target":{"kind":"service","service":"fixture","tool":"echo","idempotency_ms":null},"input":binding.input,"output":binding.output,"cases":[]});
    let image =
        Image(&bytes).replace_section(berm::abi::ABI_SECTION, &serde_json::to_vec(&record)?)?;
    host.deploy("owner/identity", &image).await?;
    let mut run = ledger_fixture::run("overflow", &image);
    run.accepted = host.host.workflow_bindings("owner/identity", "identity")?;
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
    assert!(
        host.host
            .workflows
            .pending_completions(10)
            .await?
            .is_empty()
    );
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

async fn lost_mail(host: &TestHost, keys: &[&str]) -> Result<Run> {
    let image = fixture::image("berm-fixture")?;
    host.deploy("owner/flow", &image).await?;
    let mut run = host.host.prepare_workflow(
        "owner/flow",
        "workflow_mail",
        json!({"url":"http://127.0.0.1/mail","keys":keys}),
        trigger(),
    )?;
    run.accepted.deadline_ms = 1000;
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
    host.arrange("POST", "http://127.0.0.1/mail", 200, b"accepted-email");
    let index = if keys.len() > 1 { 1 } else { 0 };
    host.host.workflows.store.run(move |conn| conn.execute_batch(&format!("CREATE TRIGGER lose_result BEFORE UPDATE ON runs WHEN json_extract(CAST(NEW.body AS TEXT),'$.checkpoints.messages.items[{index}].state.state')='succeeded' BEGIN SELECT RAISE(ABORT,'result persistence lost'); END;"))).await?;
    assert!(host.host.execute_workflow(&run.id).await.is_err());
    assert_eq!(host.fetched().len(), index + 1);
    host.host
        .workflows
        .store
        .run(|conn| conn.execute_batch("DROP TRIGGER lose_result"))
        .await?;
    Ok(run)
}

#[tokio::test]
async fn lost_email_result_keeps_payload_keys_and_completed_each_items() -> Result<()> {
    let host = TestHost::open(100).await?;
    let run = lost_mail(&host, &["first", "second", "third"]).await?;
    host.set_now(1101);
    host.host.workflows.recover(host.now()).await?;
    let old = host.host.workflows.get(&run.id).await?.unwrap();
    let first = old.checkpoints["messages"].items[0].clone();
    let result = host.host.continue_workflow(&run.id).await?;
    assert_eq!(result.status, Status::Succeeded, "{:?}", result.error);
    assert_eq!(
        result.result,
        Some(json!([
            "accepted-email",
            "accepted-email",
            "accepted-email"
        ]))
    );
    assert_eq!(result.checkpoints["messages"].items[0], first);
    assert_eq!(result.attempts.len(), 2);
    let requests = host.fetched();
    let keys = requests
        .iter()
        .map(|request| {
            request
                .headers
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case("Idempotency-Key"))
                .unwrap()
                .1
                .as_str()
        })
        .collect::<Vec<_>>();
    assert_eq!(keys, ["first", "second", "second", "third"]);
    assert_eq!(requests[1].body, requests[2].body);
    Ok(())
}

#[tokio::test]
async fn email_after_its_original_window_is_refused_with_an_explanation() -> Result<()> {
    let host = TestHost::open(100).await?;
    let run = lost_mail(&host, &["email"]).await?;
    host.set_now(10100);
    host.host.workflows.recover(host.now()).await?;
    let response = berm_host::Denied::from(host.host.continue_workflow(&run.id).await.unwrap_err())
        .into_response();
    assert_eq!(response.status(), 409);
    let body: Value = serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await?)?;
    assert_eq!(body["explain"], "E0067");
    assert_eq!(host.fetched().len(), 1);
    assert_eq!(
        host.host
            .workflows
            .get(&run.id)
            .await?
            .unwrap()
            .attempts
            .len(),
        1
    );
    Ok(())
}

#[tokio::test]
async fn cancellation_drains_one_email_and_fences_the_next_dispatch() -> Result<()> {
    let host = TestHost::open(100).await?;
    let run = admit(
        &host,
        "workflow_mail",
        json!({"url":"http://127.0.0.1/mail","keys":["first","second"]}),
    )
    .await?;
    let ready = host.fetch.respond_blocked(
        "POST",
        "http://127.0.0.1/mail",
        Response {
            status: 200,
            headers: vec![],
            body: b"accepted".to_vec(),
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
    assert_eq!(
        host.host.cancel_workflow(&run.id).await?.status,
        Status::Cancelling
    );
    ready.notify_one();
    let result = task.await??;
    assert_eq!(result.status, Status::Cancelled);
    assert!(result.result.is_none());
    assert_eq!(host.fetched().len(), 1);
    assert!(matches!(
        result.checkpoints["messages"].items[0].state,
        State::Succeeded { .. }
    ));
    assert!(result.attempts[0].meter.calls > 0);
    let completion = &host.host.workflows.pending_completions(10).await?[0];
    assert_eq!(completion.status, Status::Cancelled);
    assert_eq!(completion.succeeded.get("messages"), Some(&1));
    Ok(())
}

#[tokio::test]
async fn continuation_reuses_branch_and_items_with_the_original_image() -> Result<()> {
    let host = TestHost::open(100).await?;
    let run = admit(&host, "workflow_flow", json!({"count":3,"take":true})).await?;
    host.host.workflows.store.run(|conn|conn.execute_batch("CREATE TRIGGER stop_dispatch BEFORE UPDATE ON runs WHEN json_extract(CAST(NEW.body AS TEXT),'$.checkpoints.items.items[1].dispatches[0].state.state')='running' BEGIN SELECT RAISE(ABORT,'dispatch journal unavailable'); END;")).await?;
    assert!(host.host.execute_workflow(&run.id).await.is_err());
    host.host
        .workflows
        .store
        .run(|conn| conn.execute_batch("DROP TRIGGER stop_dispatch"))
        .await?;
    host.set_now(run.created_at + run.accepted.deadline_ms + 1);
    host.host.workflows.recover(host.now()).await?;
    let saved = host.host.workflows.get(&run.id).await?.unwrap();
    let bytes = fixture::image("berm-fixture")?;
    host.deploy(
        "owner/flow",
        &Image(&bytes).with_section("updated-image", b"new")?,
    )
    .await?;
    let (a, b) = tokio::join!(
        host.host.continue_workflow(&run.id),
        host.host.continue_workflow(&run.id)
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    let completed = a.or(b)?;
    assert_eq!(completed.status, Status::Succeeded, "{:?}", completed.error);
    assert_eq!(completed.source, run.source);
    assert_eq!(completed.checkpoints["choose"], saved.checkpoints["choose"]);
    assert_eq!(
        completed.checkpoints["items"].items[0],
        saved.checkpoints["items"].items[0]
    );
    let items: Value =
        serde_json::from_slice(&host.state("owner/flow", "workflow_items").await?.unwrap())?;
    assert_eq!(items, json!([99, 88, 0, 1, 2]));
    assert_eq!(
        host.host.cancel_workflow(&run.id).await?.status,
        Status::Succeeded
    );
    Ok(())
}

#[tokio::test]
async fn a_quote_counts_input_only_priced_steps_and_leaves_the_rest_bounded() -> Result<()> {
    let host = TestHost::open(100).await?;
    let image = fixture::image("berm-fixture")?;
    host.deploy("owner/flow", &image).await?;
    let quote = host
        .host
        .quote_workflow(
            "owner/flow",
            "workflow_priced",
            json!({"count": 2, "take": true}),
        )
        .await?;
    assert_eq!(
        quote.steps,
        [("items".to_owned(), 2), ("optional".to_owned(), 1)].into()
    );
    assert_eq!(
        quote.branches,
        [("choose".to_owned(), "yes".to_owned())].into()
    );
    assert!(quote.meter.cpu > 0);
    let graph = &host
        .host
        .workflow_offer("owner/flow", "workflow_priced")?
        .spec
        .entry
        .graph;
    assert_eq!(graph.price_range(0, &quote.steps), (900, 1200));
    let skipped = host
        .host
        .quote_workflow(
            "owner/flow",
            "workflow_priced",
            json!({"count": 0, "take": false}),
        )
        .await?;
    assert_eq!(
        skipped.steps,
        [("items".to_owned(), 0), ("optional".to_owned(), 0)].into()
    );
    assert!(
        host.host
            .quote_workflow("owner/flow", "workflow_priced", json!({"count": "two"}))
            .await
            .is_err()
    );
    Ok(())
}
