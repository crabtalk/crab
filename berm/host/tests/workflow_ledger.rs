use anyhow::Result;
use berm_api::{
    Image, Meter, digest, host,
    workflow::run::{
        Checkpoint, Dispatch, Failure, Item, Prepared, PreparedCall, Receipt, State, Status,
    },
};
use berm_host::{
    test::TestHost,
    workflow::{Eligibility, Intent, Ledger, Refused, StorageGrant},
};
use serde_json::json;
#[path = "../../api/tests/support/workflow.rs"]
mod workflow;

#[path = "support/workflow.rs"]
mod fixture;

async fn insert(ledger: &Ledger, id: &str, image: &[u8], bytes: u64) -> Result<()> {
    ledger
        .insert(
            fixture::run(id, image),
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
        body: json!({"error":"account memory allowance exhausted","explain":"E0019","recovery":{"remaining":0}}),
    }
}

fn prepared() -> Checkpoint {
    Checkpoint {
        state: State::Running,
        prepared: Some(Prepared::Calls {
            calls: vec![
                PreparedCall {
                    input: json!({"value":1,"idempotency_key":"stable-0"}),
                    operation_key: Some("stable-0".into()),
                },
                PreparedCall {
                    input: json!({"value":2,"idempotency_key":"stable-1"}),
                    operation_key: Some("stable-1".into()),
                },
            ],
        }),
        items: vec![
            Item {
                index: 0,
                state: State::Succeeded { value: json!(11) },
                receipts: vec![],
                dispatches: vec![],
            },
            Item {
                index: 1,
                state: State::Running,
                receipts: vec![],
                dispatches: vec![Dispatch {
                    launched: None,
                    operation: None,
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
    let image = fixture::image(&[]);
    insert(&host.host.workflows, "run", &image, u64::MAX).await?;
    let claim = host
        .host
        .workflows
        .claim("run", Intent::Start, 101, 200)
        .await?;
    let mut checkpoint = prepared();
    host.host
        .workflows
        .checkpoint(&claim, "send", checkpoint.clone())
        .await?;
    checkpoint.items[1].state = State::Failed { error: failure() };
    checkpoint.items[1].dispatches[0].state = State::Failed { error: failure() };
    checkpoint.items[1].dispatches[0].finished_at = Some(103);
    checkpoint.state = State::Failed { error: failure() };
    host.host
        .workflows
        .checkpoint(&claim, "send", checkpoint.clone())
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
    assert_eq!(restored.accepted, fixture::run("run", &image).accepted);
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
    let image = fixture::image(&[]);
    let capacity = fixture::CAPACITY + image.len() as u64;
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
    insert(
        &host.host.workflows,
        id,
        &image,
        capacity + fixture::CAPACITY,
    )
    .await?;
    assert_eq!(
        host.host.workflows.reserved("owner").await?,
        capacity + fixture::CAPACITY
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
    insert(ledger, "run", &fixture::image(&[]), u64::MAX).await?;
    let (a, b) = tokio::join!(
        ledger.claim("run", Intent::Start, 101, 200),
        ledger.claim("run", Intent::Start, 101, 200)
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    let claim = a.or(b)?;
    ledger.checkpoint(&claim, "send", prepared()).await?;
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
    assert!(ledger.checkpoint(&claim, "send", prepared()).await.is_err());
    assert_eq!(ledger.get("run").await?.unwrap().attempts.len(), 2);
    Ok(())
}

#[tokio::test]
async fn failed_checkpoint_writes_leave_the_preceding_full_record() -> Result<()> {
    let host = TestHost::open(0).await?;
    let ledger = &host.host.workflows;
    insert(ledger, "run", &fixture::image(&[]), u64::MAX).await?;
    let claim = ledger.claim("run", Intent::Start, 101, 200).await?;
    ledger.checkpoint(&claim, "send", prepared()).await?;
    let before = serde_json::to_value(ledger.get("run").await?)?;
    ledger.store.run(|conn|conn.execute_batch("CREATE TRIGGER refuse_checkpoint BEFORE UPDATE ON runs BEGIN SELECT RAISE(ABORT,'disk write refused'); END;")).await?;
    let mut after = prepared();
    after.state = State::Failed { error: failure() };
    assert!(ledger.checkpoint(&claim, "send", after).await.is_err());
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
    insert(ledger, "run", &fixture::image(&[]), u64::MAX).await?;
    let claim = ledger.claim("run", Intent::Start, 101, 200).await?;
    let before = serde_json::to_value(ledger.get("run").await?)?;
    let mut checkpoint = prepared();
    checkpoint.items[0].state = State::Succeeded {
        value: json!("x".repeat(fixture::CAPACITY as usize)),
    };
    let error = ledger
        .checkpoint(&claim, "send", checkpoint)
        .await
        .unwrap_err();
    assert!(matches!(error.downcast_ref(), Some(Refused::Capacity)));
    assert_eq!(serde_json::to_value(ledger.get("run").await?)?, before);
    Ok(())
}

#[tokio::test]
async fn stopped_deleted_upgraded_and_expired_services_have_explicit_eligibility() -> Result<()> {
    let mut host = TestHost::open(0).await?;
    let old = fixture::image(&[]);
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
    let changed = workflow::image(json!({"max_cost":1000,"checkpoint_bytes":fixture::CAPACITY}));
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
    let image = fixture::image(&[]);
    let bytes = fixture::CAPACITY + image.len() as u64;
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

#[tokio::test]
async fn expired_dispatch_lease_still_records_its_owned_in_flight_outcome() -> Result<()> {
    let host = TestHost::open(100).await?;
    let ledger = &host.host.workflows;
    insert(ledger, "run", &fixture::image(&[]), u64::MAX).await?;
    let claim = ledger.claim("run", Intent::Start, 101, 200).await?;
    let mut checkpoint = prepared();
    checkpoint.state = State::Prepared;
    for item in &mut checkpoint.items {
        item.state = State::Pending;
        item.dispatches.clear();
    }
    ledger
        .checkpoint(&claim, "send", checkpoint.clone())
        .await?;
    let mut dispatch = ledger.begin_dispatch(&claim, "send", 0, 199).await?;
    dispatch.state = State::Failed { error: failure() };
    dispatch.finished_at = Some(201);
    dispatch.meter.cpu = 10;
    checkpoint.items[0].state = dispatch.state.clone();
    checkpoint.items[0].dispatches.push(dispatch);
    checkpoint.state = State::Failed { error: failure() };
    ledger
        .checkpoint(&claim, "send", checkpoint.clone())
        .await?;
    assert!(ledger.begin_dispatch(&claim, "send", 1, 201).await.is_err());
    ledger
        .finish(
            &claim,
            Status::Failed,
            None,
            Some(failure()),
            Meter {
                cpu: 10,
                ..Default::default()
            },
            202,
        )
        .await?;
    let run = ledger.get("run").await?.unwrap();
    assert_eq!(run.checkpoints["send"], checkpoint);
    assert_eq!(run.attempts[0].meter.cpu, 10);
    assert_eq!(run.error, Some(failure()));
    Ok(())
}

#[tokio::test]
async fn terminal_receipts_survive_restart_deletion_and_duplicate_acknowledgement() -> Result<()> {
    let mut host = TestHost::open(0).await?;
    let image = fixture::image(&[]);
    insert(&host.host.workflows, "receipt", &image, u64::MAX).await?;
    let claim = host
        .host
        .workflows
        .claim("receipt", Intent::Start, 101, 200)
        .await?;
    assert!(
        host.host
            .workflows
            .pending_completions(10)
            .await?
            .is_empty()
    );
    host.host
        .workflows
        .finish(
            &claim,
            Status::Failed,
            None,
            Some(failure()),
            Meter::default(),
            150,
        )
        .await?;
    host.reopen().await?;
    let receipts = host.host.workflows.pending_completions(10).await?;
    assert_eq!(receipts.len(), 1);
    assert_eq!(receipts[0].attempt, claim.attempt);
    assert_eq!(receipts[0].status, Status::Failed);
    host.host.workflows.delete("receipt", 201).await?;
    assert_eq!(host.host.workflows.pending_completions(10).await?, receipts);
    host.host
        .workflows
        .acknowledge_completion("receipt", &claim.attempt)
        .await?;
    host.host
        .workflows
        .acknowledge_completion("receipt", &claim.attempt)
        .await?;
    assert!(
        host.host
            .workflows
            .pending_completions(10)
            .await?
            .is_empty()
    );
    Ok(())
}

#[tokio::test]
async fn recovering_an_expired_attempt_durably_reports_indeterminate_without_restarting()
-> Result<()> {
    let host = TestHost::open(0).await?;
    let image = fixture::image(&[]);
    insert(&host.host.workflows, "interrupted", &image, u64::MAX).await?;
    let claim = host
        .host
        .workflows
        .claim("interrupted", Intent::Start, 101, 200)
        .await?;
    host.host.workflows.recover(200).await?;
    host.host.workflows.recover(201).await?;
    let receipts = host.host.workflows.pending_completions(10).await?;
    assert_eq!(receipts.len(), 1);
    assert_eq!(receipts[0].attempt, claim.attempt);
    assert_eq!(receipts[0].status, Status::Indeterminate);
    assert_eq!(
        host.host
            .workflows
            .get("interrupted")
            .await?
            .unwrap()
            .attempts
            .len(),
        1
    );
    Ok(())
}

#[tokio::test]
async fn deleting_or_expiring_an_unreported_attempt_preserves_its_terminal_receipt() -> Result<()> {
    let host = TestHost::open(0).await?;
    let image = fixture::image(&[]);
    for id in ["deleted", "expired"] {
        insert(&host.host.workflows, id, &image, u64::MAX).await?;
        host.host
            .workflows
            .claim(id, Intent::Start, 101, 200)
            .await?;
        if id == "deleted" {
            host.host.workflows.delete(id, 200).await?;
        } else {
            host.host.workflows.expire(1100).await?;
        }
    }
    let receipts = host.host.workflows.pending_completions(10).await?;
    assert_eq!(receipts.len(), 2);
    assert!(
        receipts
            .iter()
            .all(|receipt| receipt.status == Status::Indeterminate)
    );
    assert!(host.host.workflows.get("deleted").await?.is_none());
    assert!(host.host.workflows.get("expired").await?.is_none());
    Ok(())
}

#[tokio::test]
async fn a_replay_that_never_launched_cannot_clear_the_original_uncertainty() -> Result<()> {
    let host = TestHost::open(0).await?;
    let ledger = &host.host.workflows;
    insert(ledger, "uncertain", &fixture::image(&[]), u64::MAX).await?;
    let claim = ledger.claim("uncertain", Intent::Start, 101, 200).await?;
    ledger.checkpoint(&claim, "send", prepared()).await?;
    ledger.recover(200).await?;
    let replay = ledger
        .claim("uncertain", Intent::Continue, 201, 300)
        .await?;
    let dispatch = ledger.begin_dispatch(&replay, "send", 1, 202).await?;
    assert_eq!(dispatch.operation.as_deref(), Some("dispatch-1"));
    ledger.recover(300).await?;
    let run = ledger.get("uncertain").await?.unwrap();
    let Some(Prepared::Calls { calls }) = &run.checkpoints["send"].prepared else {
        panic!("missing calls")
    };
    let mut error = failure();
    error.headers.insert(host::LAUNCHED.into(), "false".into());
    ledger
        .reconcile(
            "uncertain",
            Receipt {
                dispatch: dispatch.id,
                input: digest::json_hex(&calls[1].input),
                binding: digest::json_hex(&serde_json::to_value(&run.accepted.bindings["send"])?),
                outcome: State::Failed { error },
            },
            301,
        )
        .await?;
    assert!(matches!(
        ledger.get("uncertain").await?.unwrap().checkpoints["send"].items[1].state,
        State::Unknown { .. }
    ));
    let next = ledger
        .claim("uncertain", Intent::Continue, 302, 400)
        .await?;
    let dispatch = ledger.begin_dispatch(&next, "send", 1, 303).await?;
    assert_eq!(dispatch.operation.as_deref(), Some("dispatch-1"));
    Ok(())
}

#[tokio::test]
async fn known_failures_cannot_extend_the_prepared_calls_idempotency_window() -> Result<()> {
    for launched in [true, false] {
        let host = TestHost::open(0).await?;
        let ledger = &host.host.workflows;
        let image = fixture::image(&[]);
        let mut record: serde_json::Value =
            serde_json::from_slice(Image(&image).section(berm::abi::ABI_SECTION)?.unwrap())?;
        record["workflow"]["graph"]["nodes"][0]["call"]["target"]["idempotency_ms"] = json!(100);
        let image =
            Image(&image).replace_section(berm::abi::ABI_SECTION, &serde_json::to_vec(&record)?)?;
        let mut run = fixture::run("window", &image);
        run.accepted
            .bindings
            .get_mut("send")
            .unwrap()
            .idempotency_ms = Some(100);
        ledger
            .insert(
                run.clone(),
                image,
                StorageGrant {
                    scope: "owner".into(),
                    bytes: u64::MAX,
                },
            )
            .await?;
        let claim = ledger.claim("window", Intent::Start, 101, 200).await?;
        let mut checkpoint = prepared();
        checkpoint.state = State::Failed { error: failure() };
        let item = &mut checkpoint.items[1];
        item.state = State::Failed { error: failure() };
        item.dispatches[0].state = item.state.clone();
        item.dispatches[0].finished_at = Some(110);
        item.dispatches[0].launched = Some(true);
        ledger.checkpoint(&claim, "send", checkpoint).await?;
        ledger
            .finish(
                &claim,
                Status::Failed,
                None,
                Some(failure()),
                Meter::default(),
                110,
            )
            .await?;
        let next = ledger.claim("window", Intent::Continue, 150, 250).await?;
        let dispatch = ledger.begin_dispatch(&next, "send", 1, 151).await?;
        assert!(dispatch.operation.is_none());
        let mut checkpoint = ledger.get("window").await?.unwrap().checkpoints["send"].clone();
        let item = &mut checkpoint.items[1];
        item.state = State::Failed { error: failure() };
        let dispatch = item.dispatches.last_mut().unwrap();
        dispatch.state = item.state.clone();
        dispatch.finished_at = Some(160);
        dispatch.launched = Some(launched);
        checkpoint.state = State::Failed { error: failure() };
        ledger.checkpoint(&next, "send", checkpoint).await?;
        ledger
            .finish(
                &next,
                Status::Failed,
                None,
                Some(failure()),
                Meter::default(),
                160,
            )
            .await?;
        let refusal = ledger
            .claim("window", Intent::Continue, 203, 303)
            .await
            .unwrap_err();
        assert!(matches!(refusal.downcast_ref(), Some(Refused::Unsafe(_))));
        assert_eq!(ledger.get("window").await?.unwrap().attempts.len(), 2);
    }
    Ok(())
}
