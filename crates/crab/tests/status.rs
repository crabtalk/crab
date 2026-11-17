use axum::{Json, Router, http::StatusCode, routing::get};
use serde_json::{Value, json};
use std::{
    fs,
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use tokio::{net::TcpListener, task};

#[tokio::test(flavor = "multi_thread")]
async fn status_reads_its_own_credential_without_account_read_permissions() {
    for scopes in [json!([]), json!(["usage:read"])] {
        let reads = Arc::new(AtomicUsize::new(0));
        let credential = json!({"kind":"api_key","key":{
            "id":"key-1","label":"agent","owner_id":"bound-account","scope":scopes,
            "expires_at":null,"budget":5_000_000,"spent":123,"services":[]
        }});
        let app = Router::new()
            .route("/meta/health", get(|| async { Json(json!({"ok":true})) }))
            .route(
                "/auth/status",
                get({
                    let credential = credential.clone();
                    move || async move { Json(credential) }
                }),
            )
            .route(
                "/orgs/selected/billing/usage",
                get({
                    let reads = reads.clone();
                    move || {
                        let reads = reads.clone();
                        async move {
                            reads.fetch_add(1, Ordering::SeqCst);
                            (
                                StatusCode::FORBIDDEN,
                                Json(json!({"error":"outside key account"})),
                            )
                        }
                    }
                }),
            );
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join(".crab")).unwrap();
        fs::write(
            root.path().join(".crab/contexts.toml"),
            format!(
                r#"
current = "test"
[contexts.test]
host = "http://{address}"
token = "test-credential"
owner = {{ user = "configured-account" }}
"#
            ),
        )
        .unwrap();
        let home = root.path().to_owned();
        let output = task::spawn_blocking(move || {
            Command::new(env!("CARGO_BIN_EXE_crab"))
                .env("HOME", home)
                .args(["--org", "selected", "status", "--json"])
                .output()
                .unwrap()
        })
        .await
        .unwrap();
        server.abort();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let value: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(value["account"], "selected");
        assert_eq!(value["credential"], credential);
        assert!(value.get("plan").is_none());
        assert_eq!(
            reads.load(Ordering::SeqCst),
            usize::from(!scopes.as_array().unwrap().is_empty())
        );
        assert!(!String::from_utf8_lossy(&output.stdout).contains("test-credential"));
    }
}
