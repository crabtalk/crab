use anyhow::Result;
use axum::{body::to_bytes, http::StatusCode, response::IntoResponse};
use berm::{Enter, Meters, control, workflow as spend};
use berm_api::workflow::{run::Failure, spend::Context as SpendContext};
use berm_build::fixture;
use berm_host::{Config, test::TestHost};
use serde_json::{Value, json};
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
#[path = "../../api/tests/support/workflow.rs"]
mod workflow;

fn image(body: &str, imports: &str, data: &str, budget: Value) -> Vec<u8> {
    let wasm = wat::parse_str(format!(
        "(module {imports} (memory (export \"memory\") 1) {data} (func (export \"berm_workflow_identity_done\") {body}))"
    )).unwrap();
    workflow::with_wasm(
        &wasm,
        json!({"max_cost":1000,"checkpoint_bytes":4096,"budget":budget}),
    )
}

async fn execute(
    host: &TestHost,
    input: &berm::abi::workflow::ControlInput,
    meters: Arc<Meters>,
) -> Result<berm::abi::workflow::ControlOutput, berm_host::Denied> {
    host.host
        .control(
            "identity",
            control::Request {
                workflow: "identity",
                node: "done",
                input,
                deadline: Instant::now() + Duration::from_secs(2),
                meters,
            },
        )
        .await
}

#[tokio::test]
async fn helpers_cannot_reach_application_syscalls_and_explanations_survive() -> Result<()> {
    for syscall in [
        berm::abi::CALL,
        berm::abi::CALL_AFTER,
        berm::abi::HTTP_FETCH,
        berm::abi::SQL,
        berm::abi::GET,
        berm::abi::SET,
        berm::abi::EMIT,
        berm::abi::NOW,
        berm::abi::RANDOM,
        berm::abi::VAR,
        berm::abi::CALLER,
        berm::abi::WORKFLOW_RUN,
        berm::abi::SERVICE,
        berm::abi::DEADLINE,
        berm::abi::CALL_ID,
    ] {
        let host = TestHost::open(0).await?;
        let imports =
            format!("(import \"berm\" \"{syscall}\" (func $effect (param i32 i32) (result i64)))");
        let data = "(func $helper i32.const 0 i32.const 0 call $effect drop)";
        let image = image("call $helper", &imports, data, json!({}));
        host.deploy("identity", &image).await?;
        let input = berm::abi::workflow::ControlInput {
            input: json!(0),
            nodes: Default::default(),
        };
        let response = execute(&host, &input, Arc::default())
            .await
            .unwrap_err()
            .into_response();
        assert_eq!(
            response.status(),
            StatusCode::UNPROCESSABLE_ENTITY,
            "{syscall}"
        );
        let body: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await?)?;
        assert_eq!(
            body["explain"],
            control::Effect::EXPLAIN,
            "{syscall}: {body}"
        );
        assert!(body["error"].as_str().unwrap().contains(syscall), "{body}");
        assert!(host.fetched().is_empty());
    }
    Ok(())
}

#[tokio::test]
async fn real_rust_adapters_run_without_opening_application_state() -> Result<()> {
    let host = TestHost::open(0).await?;
    host.deploy("mixed", &fixture::image("berm-fixture")?)
        .await?;
    let input = berm::abi::workflow::ControlInput {
        input: json!(2),
        nodes: Default::default(),
    };
    let result = host
        .host
        .control(
            "mixed",
            control::Request {
                workflow: "workflow_example",
                node: "spin",
                input: &input,
                deadline: Instant::now() + Duration::from_secs(2),
                meters: Arc::default(),
            },
        )
        .await?;
    let berm::abi::workflow::ControlOutput::Calls { inputs } = result else {
        panic!("not a builder result")
    };
    assert_eq!(inputs, [json!({"rounds":2})]);
    Ok(())
}

#[tokio::test]
async fn traps_cpu_exhaustion_and_oversized_buffers_fail_cleanly() -> Result<()> {
    let input = berm::abi::workflow::ControlInput {
        input: json!(0),
        nodes: Default::default(),
    };
    for (body, imports, budget, expected) in [
        ("unreachable", "", json!({}), "unreachable"),
        ("(loop $spin br $spin)", "", json!({"cpu":1000}), "CPU"),
        (
            "i32.const 0 i32.const 32768 call $done drop",
            "(import \"berm\" \"berm.done\" (func $done (param i32 i32) (result i64)))",
            json!({}),
            "output buffer",
        ),
    ] {
        let host = TestHost::open(0).await?;
        host.deploy("identity", &image(body, imports, "", budget))
            .await?;
        let error = execute(&host, &input, Arc::default()).await.unwrap_err();
        assert!(error.message.contains(expected), "{}", error.message);
    }
    let host = TestHost::open(0).await?;
    host.deploy("identity", &image("", "", "", json!({})))
        .await?;
    let input = berm::abi::workflow::ControlInput {
        input: json!("x".repeat(16384)),
        nodes: Default::default(),
    };
    assert!(
        execute(&host, &input, Arc::default())
            .await
            .unwrap_err()
            .message
            .contains("input buffer")
    );
    Ok(())
}

#[tokio::test]
async fn mutable_globals_are_fresh_for_every_control_invocation() -> Result<()> {
    let host = TestHost::open(0).await?;
    let imports = "(import \"berm\" \"berm.done\" (func $done (param i32 i32) (result i64)))";
    let output = r#"{\"kind\":\"output\",\"value\":1}"#;
    let changed = r#"{\"kind\":\"output\",\"value\":2}"#;
    let data = format!(
        "(global $count (mut i32) (i32.const 0)) (data (i32.const 0) \"{output}\") (data (i32.const 64) \"{changed}\")"
    );
    let body = "global.get $count i32.const 1 i32.add global.set $count global.get $count i32.const 1 i32.eq (if (result i32) (then i32.const 0) (else i32.const 64)) i32.const 27 call $done drop";
    host.deploy("identity", &image(body, imports, &data, json!({})))
        .await?;
    let input = berm::abi::workflow::ControlInput {
        input: json!(0),
        nodes: Default::default(),
    };
    for _ in 0..3 {
        let output = execute(&host, &input, Arc::default()).await?;
        let berm::abi::workflow::ControlOutput::Output { value } = output else {
            panic!("not output")
        };
        assert_eq!(value, 1);
    }
    Ok(())
}

#[tokio::test]
async fn control_memory_and_deadline_limits_are_metered() -> Result<()> {
    let host = TestHost::open(0).await?;
    let body = "i32.const 64 memory.grow drop i32.const 2097152 i32.const 1 i32.store";
    host.deploy("identity", &image(body, "", "", json!({"memory":1})))
        .await?;
    let input = berm::abi::workflow::ControlInput {
        input: json!(0),
        nodes: Default::default(),
    };
    let meters = Arc::new(Meters::default());
    let error = execute(&host, &input, meters.clone()).await.unwrap_err();
    assert!(error.message.contains("memory"), "{}", error.message);
    let used = meters.take();
    let meter = used.values().next().unwrap();
    assert_eq!(meter.calls, 1);
    assert_eq!(meter.failed, 1);
    assert!(meter.memory <= 1 << 20);
    let error = host
        .host
        .control(
            "identity",
            control::Request {
                workflow: "identity",
                node: "done",
                input: &input,
                deadline: Instant::now() - Duration::from_millis(1),
                meters: Arc::default(),
            },
        )
        .await
        .unwrap_err();
    assert!(error.message.contains("deadline"), "{}", error.message);
    Ok(())
}

#[tokio::test]
async fn account_memory_refusals_keep_the_existing_status_and_explanation() -> Result<()> {
    let mut host = TestHost::open(0).await?;
    host.deploy(
        "acct/identity",
        &workflow::image(json!({"max_cost":1000,"checkpoint_bytes":4096})),
    )
    .await?;
    host.reopen().await?;
    host.host.limit([("acct".to_owned(), 1)].into());
    let input = berm::abi::workflow::ControlInput {
        input: json!(0),
        nodes: Default::default(),
    };
    let error = host
        .host
        .control(
            "acct/identity",
            control::Request {
                workflow: "identity",
                node: "done",
                input: &input,
                deadline: Instant::now() + Duration::from_secs(2),
                meters: Arc::default(),
            },
        )
        .await
        .unwrap_err();
    assert_eq!(error.status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(error.explain, Some("E0019"));
    Ok(())
}

#[tokio::test]
async fn control_admission_has_its_budget_and_spend_context_and_preserves_full_refusals()
-> Result<()> {
    let seen = Arc::new(Mutex::new(None));
    let observed = seen.clone();
    let enter: Enter = Arc::new(move |_, at| {
        *observed.lock().unwrap() = Some(at.clone());
        Box::pin(async {
            Err(Failure {
            status: 402,headers: [("retry-after".into(),"3".into())].into(),
            body: json!({"error":"cap exhausted","explain":"E0061","recovery":{"next":"inspect"}}),
        }.into())
        })
    });
    let host = TestHost::open_with_scope(0, Config::default(), Some(enter)).await?;
    host.deploy("identity", &image("", "", "", json!({"cpu":1234})))
        .await?;
    let context = SpendContext {
        run: "run".into(),
        attempt: "attempt".into(),
        owner: "owner".into(),
        deadline_at: 100_000,
        authorization: "private-grant".into(),
    };
    let input = berm::abi::workflow::ControlInput {
        input: json!(0),
        nodes: Default::default(),
    };
    let denied = spend::scope(context.clone(), execute(&host, &input, Arc::default()))
        .await
        .unwrap_err();
    let response = denied.into_response();
    assert_eq!(response.status(), StatusCode::PAYMENT_REQUIRED);
    assert_eq!(response.headers()["retry-after"], "3");
    let body: Value = serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await?)?;
    assert_eq!(body["explain"], "E0061");
    assert_eq!(body["recovery"]["next"], "inspect");
    let at = seen.lock().unwrap().take().unwrap();
    assert!(at.control);
    assert_eq!(at.budget.cpu, 1234);
    assert_eq!(at.workflow, Some(context));
    assert!(host.state("identity", "anything").await?.is_none());
    Ok(())
}
