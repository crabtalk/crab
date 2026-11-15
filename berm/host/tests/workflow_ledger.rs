use anyhow::Result;
use berm_api::{
    Budget, Manifest, Meter, digest,
    workflow::{
        Version,
        graph::{Kind, Source},
        run::{
            Accepted, Binding, Checkpoint, Dispatch, Failure, Item, Prepared, PreparedCall, Run,
            State, Status, Trigger,
        },
    },
};
use berm_host::{
    test::TestHost,
    workflow::{Eligibility, Intent, Ledger, Refused, StorageGrant},
};
use serde_json::json;
#[path = "../../api/tests/support/workflow.rs"]
mod workflow;

const CAPACITY: u64 = 32768;

fn image() -> Vec<u8> {
    let wasm = wat::parse_str("(module (memory (export \"memory\") 1) (func (export \"berm_workflow_identity_send\")) (func (export \"berm_workflow_identity_done\")))").unwrap();
    let schema = json!({"type":"integer"});
    workflow::with_record(
        &wasm,
        json!({
            "workflow":{"version":1,"name":"identity","graph":{
                "input":schema,"output":{"type":"array","items":schema},
                "nodes":[
                    {"id":"send","export":"berm_workflow_identity_send","kind":"step","call":{"target":{"kind":"service","service":"mail/send","tool":"send","idempotency_ms":10000},"input":schema,"output":schema,"cases":[]},"mode":{"kind":"each","max":3},"dependencies":[{"source":{"kind":"input"},"optional":false,"case":null}],"guards":[]},
                    {"id":"done","export":"berm_workflow_identity_done","kind":"output","dependencies":[{"source":{"kind":"node","id":"send"},"optional":false,"case":null}],"guards":[]}
                ]}},"abi_version":berm::abi::ABI_VERSION,"crate":"workflow-only"
        }),
        json!({"max_cost":1000,"checkpoint_bytes":CAPACITY,"retain":"1s"}),
    )
}

fn run(id: &str, image: &[u8]) -> Run {
    let manifest = Manifest::try_from(image).unwrap();
    let spec = &manifest.workflows[0];
    let budget = Budget {
        cpu: 1000,
        read: 0,
        written: 0,
        memory: 1 << 20,
    };
    let bindings = spec
        .entry
        .graph
        .nodes
        .iter()
        .filter_map(|node| match &node.kind {
            Kind::Step { call, .. } => Some((
                node.id.clone(),
                Binding {
                    target: call.target.clone(),
                    service: "mail/send".into(),
                    image: "accepted-provider-image".into(),
                    input: call.input.clone(),
                    output: call.output.clone(),
                    budget,
                    authorization: "revision-1".into(),
                },
            )),
            _ => None,
        })
        .collect();
    Run {
        version: Version::V1,
        id: id.into(),
        service: "owner/identity".into(),
        workflow: "identity".into(),
        source: Source {
            image: digest::of(image),
            graph: spec.entry.digest().unwrap(),
        },
        input: json!(2),
        trigger: Trigger::Schedule {
            key: format!("occurrence-{id}"),
            at: 100,
        },
        created_at: 100,
        expires_at: 1100,
        status: Status::Pending,
        accepted: Accepted {
            bindings,
            control_budget: budget,
            control_buffer: manifest.buffer as u64,
            checkpoint_bytes: CAPACITY,
            deadline_ms: 100,
        },
        attempts: vec![],
        checkpoints: Default::default(),
        result: None,
        error: None,
    }
}

async fn insert(ledger: &Ledger, id: &str, image: &[u8], bytes: u64) -> Result<()> {
    ledger
        .insert(
            run(id, image),
            image.to_vec(),
            StorageGrant {
                scope: "owner".into(),
                bytes,
            },
        )
        .await
}

fn failure() -> Failure {
    Failure {
        status: 429,
        headers: [("retry-after".into(), "3".into())].into(),
        body: json!({"error":"owner cap exhausted","explain":"E0060","recovery":{"remaining":0}}),
    }
}

fn prepared() -> Checkpoint {
    Checkpoint {
        state: State::Running,
        prepared: Some(Prepared::Calls {
            calls: vec![
                PreparedCall {
                    input: json!(1),
                    operation_key: Some("stable-0".into()),
                },
                PreparedCall {
                    input: json!(2),
                    operation_key: Some("stable-1".into()),
                },
            ],
        }),
        items: vec![
            Item {
                index: 0,
                state: State::Succeeded { value: json!(11) },
                dispatches: vec![],
            },
            Item {
                index: 1,
                state: State::Running,
                dispatches: vec![Dispatch {
                    id: "dispatch-1".into(),
                    attempt: "attempt-1".into(),
                    started_at: 102,
                    finished_at: None,
                    state: State::Running,
                    meter: Meter::default(),
                }],
            },
        ],
        meter: Meter {
            cpu: 123,
            ..Default::default()
        },
    }
}

#[tokio::test]
async fn restart_retains_complete_values_bindings_order_and_errors() -> Result<()> {
    let mut host = TestHost::open(0).await?;
    let image = image();
    insert(&host.host.workflows, "run", &image, u64::MAX).await?;
    let claim = host
        .host
        .workflows
        .claim("run", Intent::Start, 101, 200)
        .await?;
    let mut checkpoint = prepared();
    host.host
        .workflows
        .checkpoint(&claim, "send", checkpoint.clone(), 102)
        .await?;
    checkpoint.items[1].state = State::Failed { error: failure() };
    checkpoint.items[1].dispatches[0].state = State::Failed { error: failure() };
    checkpoint.items[1].dispatches[0].finished_at = Some(103);
    checkpoint.state = State::Failed { error: failure() };
    host.host
        .workflows
        .checkpoint(&claim, "send", checkpoint.clone(), 103)
        .await?;
    host.host
        .workflows
        .finish(
            &claim,
            Status::Failed,
            None,
            Some(failure()),
            Meter {
                cpu: 321,
                ..Default::default()
            },
            104,
        )
        .await?;
    host.reopen().await?;
    let restored = host.host.workflows.get("run").await?.unwrap();
    assert_eq!(restored.checkpoints["send"], checkpoint);
    assert_eq!(restored.error, Some(failure()));
    assert_eq!(restored.accepted, run("run", &image).accepted);
    assert_eq!(restored.attempts[0].meter.cpu, 321);
    assert_eq!(host.host.workflows.image("run").await?, image);
    let synchronous: i64 = host
        .host
        .workflows
        .store
        .run(|conn| conn.pragma_query_value(None, "synchronous", |row| row.get(0)))
        .await?;
    assert_eq!(synchronous, 2);
    Ok(())
}

#[tokio::test]
async fn reservations_are_atomic_and_retained_images_are_charged_once_per_scope() -> Result<()> {
    let host = TestHost::open(0).await?;
    let image = image();
    let capacity = CAPACITY + image.len() as u64;
    let (a, b) = tokio::join!(
        insert(&host.host.workflows, "a", &image, capacity),
        insert(&host.host.workflows, "b", &image, capacity)
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    let error = a.err().or_else(|| b.err()).unwrap();
    assert!(matches!(error.downcast_ref(), Some(Refused::Capacity)));
    assert_eq!(host.host.workflows.reserved("owner").await?, capacity);
    let id = if host.host.workflows.get("a").await?.is_some() {
        "b"
    } else {
        "a"
    };
    insert(&host.host.workflows, id, &image, capacity + CAPACITY).await?;
    assert_eq!(
        host.host.workflows.reserved("owner").await?,
        capacity + CAPACITY
    );
    host.host.workflows.delete("a", 101).await?;
    assert_eq!(host.host.workflows.reserved("owner").await?, capacity);
    host.host.workflows.delete("b", 101).await?;
    assert_eq!(host.host.workflows.reserved("owner").await?, 0);
    assert!(host.host.workflows.image("a").await.is_err());
    Ok(())
}

#[tokio::test]
async fn claims_exclude_workers_and_recovery_fences_the_expired_attempt() -> Result<()> {
    let host = TestHost::open(0).await?;
    let ledger = &host.host.workflows;
    insert(ledger, "run", &image(), u64::MAX).await?;
    let (a, b) = tokio::join!(
        ledger.claim("run", Intent::Start, 101, 200),
        ledger.claim("run", Intent::Start, 101, 200)
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    let claim = a.or(b)?;
    ledger.checkpoint(&claim, "send", prepared(), 102).await?;
    assert!(ledger.delete("run", 103).await.is_err());
    assert_eq!(ledger.recover(199).await?, 0);
    assert_eq!(ledger.recover(200).await?, 1);
    let restored = ledger.get("run").await?.unwrap();
    assert_eq!(restored.status, Status::Indeterminate);
    assert!(matches!(
        restored.checkpoints["send"].items[1].state,
        State::Unknown { .. }
    ));
    assert!(matches!(
        restored.checkpoints["send"].items[0].state,
        State::Succeeded { .. }
    ));
    let next = ledger.claim("run", Intent::Continue, 201, 300).await?;
    assert_ne!(claim.token, next.token);
    assert!(
        ledger
            .checkpoint(&claim, "send", prepared(), 202)
            .await
            .is_err()
    );
    assert_eq!(ledger.get("run").await?.unwrap().attempts.len(), 2);
    Ok(())
}

#[tokio::test]
async fn failed_checkpoint_writes_leave_the_preceding_full_record() -> Result<()> {
    let host = TestHost::open(0).await?;
    let ledger = &host.host.workflows;
    insert(ledger, "run", &image(), u64::MAX).await?;
    let claim = ledger.claim("run", Intent::Start, 101, 200).await?;
    ledger.checkpoint(&claim, "send", prepared(), 102).await?;
    let before = serde_json::to_value(ledger.get("run").await?)?;
    ledger.store.run(|conn|conn.execute_batch("CREATE TRIGGER refuse_checkpoint BEFORE UPDATE ON runs BEGIN SELECT RAISE(ABORT,'disk write refused'); END;")).await?;
    let mut after = prepared();
    after.state = State::Failed { error: failure() };
    assert!(ledger.checkpoint(&claim, "send", after, 103).await.is_err());
    assert_eq!(serde_json::to_value(ledger.get("run").await?)?, before);
    ledger
        .store
        .run(|conn| conn.execute_batch("DROP TRIGGER refuse_checkpoint"))
        .await?;
    assert_eq!(ledger.recover(200).await?, 1);
    assert!(matches!(
        ledger.get("run").await?.unwrap().checkpoints["send"].items[1].state,
        State::Unknown { .. }
    ));
    Ok(())
}

#[tokio::test]
async fn oversized_checkpoint_is_refused_without_truncation() -> Result<()> {
    let host = TestHost::open(0).await?;
    let ledger = &host.host.workflows;
    insert(ledger, "run", &image(), u64::MAX).await?;
    let claim = ledger.claim("run", Intent::Start, 101, 200).await?;
    let before = serde_json::to_value(ledger.get("run").await?)?;
    let mut checkpoint = prepared();
    checkpoint.items[0].state = State::Succeeded {
        value: json!("x".repeat(CAPACITY as usize)),
    };
    let error = ledger
        .checkpoint(&claim, "send", checkpoint, 102)
        .await
        .unwrap_err();
    assert!(matches!(error.downcast_ref(), Some(Refused::Capacity)));
    assert_eq!(serde_json::to_value(ledger.get("run").await?)?, before);
    Ok(())
}

#[tokio::test]
async fn stopped_deleted_upgraded_and_expired_services_have_explicit_eligibility() -> Result<()> {
    let mut host = TestHost::open(0).await?;
    let old = image();
    host.deploy("owner/identity", &old).await?;
    insert(&host.host.workflows, "run", &old, u64::MAX).await?;
    let claim = host
        .host
        .workflows
        .claim("run", Intent::Start, 101, 200)
        .await?;
    host.host
        .workflows
        .finish(
            &claim,
            Status::Failed,
            None,
            Some(failure()),
            Meter::default(),
            102,
        )
        .await?;
    assert_eq!(
        host.host.workflow_eligibility("run", 103).await?,
        Eligibility::Available
    );
    host.host.stop("owner/identity").await?;
    assert_eq!(
        host.host.workflow_eligibility("run", 103).await?,
        Eligibility::Stopped
    );
    let changed = workflow::image(json!({"max_cost":1000,"checkpoint_bytes":CAPACITY}));
    host.deploy("owner/identity", &changed).await?;
    assert_eq!(host.host.workflows.image("run").await?, old);
    host.host.remove("owner/identity").await?;
    host.deploy("owner/identity", &changed).await?;
    host.reopen().await?;
    assert_eq!(
        host.host.workflow_eligibility("run", 103).await?,
        Eligibility::Removed
    );
    assert_eq!(host.host.workflows.image("run").await?, old);
    assert!(
        host.host
            .workflows
            .claim("run", Intent::Continue, 103, 200)
            .await
            .is_err()
    );
    insert(&host.host.workflows, "expiry", &old, u64::MAX).await?;
    assert_eq!(
        host.host
            .workflows
            .checkpoint_eligibility("expiry", 1100)
            .await?,
        Eligibility::Expired
    );
    assert_eq!(host.host.workflows.expire(1100).await?, 2);
    assert_eq!(host.host.workflows.reserved("owner").await?, 0);
    Ok(())
}

#[tokio::test]
async fn independent_connections_share_claim_and_capacity_exclusion() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let first = Ledger::open(&dir.path().join("ledger.db"))?;
    let second = Ledger::open(&dir.path().join("ledger.db"))?;
    let image = image();
    let bytes = CAPACITY + image.len() as u64;
    let (a, b) = tokio::join!(
        insert(&first, "a", &image, bytes),
        insert(&second, "b", &image, bytes)
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    let id = if a.is_ok() { "a" } else { "b" };
    assert!(matches!(
        a.err().or_else(|| b.err()).unwrap().downcast_ref(),
        Some(Refused::Capacity)
    ));
    let (a, b) = tokio::join!(
        first.claim(id, Intent::Start, 101, 200),
        second.claim(id, Intent::Start, 101, 200)
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    assert!(matches!(
        a.err().or_else(|| b.err()).unwrap().downcast_ref(),
        Some(Refused::Busy)
    ));
    Ok(())
}
