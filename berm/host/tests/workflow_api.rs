use anyhow::Result;
use axum::{
    Extension,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
    response::IntoResponse,
};
use berm_api::{
    host::Caller,
    workflow::{
        run::Status,
        view::{Inspection, Record, Start},
    },
};
use berm_build::fixture;
use berm_host::{
    Denied, Named,
    test::TestHost,
    workflow::{Intent, StorageGrant},
};
use serde_json::{Value, json};
use tower::ServiceExt;
use uuid::Uuid;

fn caller() -> Caller {
    Caller {
        id: "operator".into(),
        key: None,
        app: None,
        user: None,
        client: None,
        agent: None,
    }
}

#[tokio::test]
async fn local_http_admission_is_effect_free_until_start_and_transport_replays_do_not_restart()
-> Result<()> {
    let host = TestHost::open(100).await?;
    let image = fixture::image("berm-fixture")?;
    host.deploy("flow", &image).await?;
    let priced = host
        .host
        .inspect_workflow("flow", "workflow_example", Some(&caller()))?;
    assert_eq!(priced.task_price, Some(0));
    let app = berm_host::router(host.host.clone())
        .layer(Extension(Named("flow".into())))
        .layer(Extension(caller()));
    let inspection = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/workflows/workflow_flow")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(inspection.status(), StatusCode::OK);
    assert_eq!(
        inspection.headers()[header::CACHE_CONTROL],
        "private, no-store"
    );
    let inspection: Inspection =
        serde_json::from_slice(&to_bytes(inspection.into_body(), usize::MAX).await?)?;
    assert_eq!(host.host.workflows.reserved("").await?, 0);
    let request = Start {
        id: Uuid::new_v4().to_string(),
        quote: inspection.quote.clone(),
        input: json!({"count":2,"take":true}),
    };
    let send = || {
        Request::builder()
            .method("POST")
            .uri("/workflows/workflow_flow/runs")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(serde_json::to_vec(&request).unwrap()))
            .unwrap()
    };
    let (a, b) = tokio::join!(app.clone().oneshot(send()), app.clone().oneshot(send()));
    for answer in [a?, b?] {
        assert_eq!(
            answer.status(),
            StatusCode::ACCEPTED,
            "{}",
            String::from_utf8_lossy(&to_bytes(answer.into_body(), usize::MAX).await?)
        );
    }
    let mut complete = None;
    for _ in 0..500 {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/workflows/workflow_flow/runs/{}", request.id))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(response.status(), StatusCode::OK);
        let record: Record =
            serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await?)?;
        if record.execution.status == Status::Succeeded {
            complete = Some(record);
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    let complete = complete.expect("workflow finishes");
    assert_eq!(complete.accepted.quote, inspection.quote);
    assert_eq!(complete.execution.attempts.len(), 1);
    let encoded = serde_json::to_value(&complete)?;
    assert!(encoded["attempts"][0].get("meter").is_none());
    let response = app.oneshot(send()).await?;
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    assert_eq!(
        host.host
            .workflows
            .get(&request.id)
            .await?
            .unwrap()
            .attempts
            .len(),
        1
    );
    Ok(())
}

#[tokio::test]
async fn one_claim_cannot_launch_two_workers_and_retained_data_expiry_is_refused() -> Result<()> {
    let host = TestHost::open(100).await?;
    let image = fixture::image("berm-fixture")?;
    host.deploy("flow", &image).await?;
    let run = host.host.prepare_workflow(
        "flow",
        "workflow_flow",
        json!({"count":1,"take":false}),
        berm_api::workflow::run::Trigger::Manual { caller: caller() },
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
    let claim = host.host.claim_workflow(&run.id, Intent::Start).await?;
    let (a, b) = tokio::join!(
        host.host.workflows.launch(&claim, 100),
        host.host.workflows.launch(&claim, 100)
    );
    assert_ne!(a?, b?);
    assert!(
        host.host
            .workflows
            .view(&run.id, run.expires_at)
            .await
            .is_err()
    );
    Ok(())
}

#[tokio::test]
async fn deleted_and_expired_admission_ids_survive_as_payload_free_identity_fences() -> Result<()> {
    for expire in [false, true] {
        let host = TestHost::open(100).await?;
        host.deploy("flow", &fixture::image("berm-fixture")?)
            .await?;
        let inspection = host
            .host
            .inspect_workflow("flow", "workflow_flow", Some(&caller()))?;
        let request = Start {
            id: Uuid::new_v4().to_string(),
            quote: inspection.quote,
            input: json!({"count":1,"take":false}),
        };
        host.host
            .start_local_workflow("flow", "workflow_flow", request.clone(), caller())
            .await?;
        let mut complete = None;
        for _ in 0..500 {
            let run = host.host.workflows.get(&request.id).await?.unwrap();
            if run.status == Status::Succeeded {
                complete = Some(run);
                break;
            }
            assert!(
                matches!(run.status, Status::Pending | Status::Running),
                "{:?}",
                run.error
            );
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        let run = complete.expect("workflow finishes");
        if expire {
            host.set_now(run.expires_at);
            assert_eq!(host.host.workflows.expire(run.expires_at).await?, 1);
        } else {
            assert!(host.host.workflows.delete(&request.id, 100).await?);
        }
        assert!(host.host.workflows.get(&request.id).await?.is_none());
        assert_eq!(host.host.workflows.reserved("").await?, 0);
        host.host.remove("flow").await?;
        let response = Denied::from(
            host.host
                .start_local_workflow("flow", "workflow_flow", request.clone(), caller())
                .await
                .unwrap_err(),
        )
        .into_response();
        assert_eq!(response.status(), StatusCode::GONE);
        let body: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await?)?;
        assert_eq!(body["explain"], "E0069");
        assert!(host.host.workflows.get(&request.id).await?.is_none());
        let payloads: i64 = host
            .host
            .workflows
            .store
            .read(|conn| conn.query_row("SELECT COUNT(*) FROM admissions", [], |row| row.get(0)))
            .await?;
        assert_eq!(payloads, 0);
    }
    Ok(())
}
