use axum::{Json, Router, routing::get};
use serde_json::{Value, json};
use std::{fs, process::Command};
use tokio::{net::TcpListener, task};

#[tokio::test(flavor = "multi_thread")]
async fn pricing_commands_show_included_usage_and_one_set_of_rates() {
    let list = json!({
        "runs": 120_000_000, "overage": 24_000, "read": 120_000,
        "written": 120_000_000, "held": 16, "rotating": 1_000_000
    });
    let catalog = json!([
        {
            "slug": "free", "accounts": ["user"], "usage_micros": 1_000_000,
            "price_micros": null, "included": {}, "memory": 0
        },
        {
            "slug": "go", "accounts": ["user", "org"], "usage_micros": 5_000_000,
            "price_micros": 5_000_000, "included": {}, "memory": 0
        }
    ]);
    let metering = json!({"rates": list, "included_cpu": 100, "max_log": 16384});
    let app = Router::new()
        .route("/meta/plans", get(move || async { Json(catalog) }))
        .route("/meta/rates", get(move || async { Json(metering) }));
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
owner = {{ user = "test-account" }}
"#
        ),
    )
    .unwrap();
    let home = root.path().to_owned();
    let [plans_json, rates_json] = task::spawn_blocking(move || {
        ["plans", "rates"].map(|command| {
            let output = Command::new(env!("CARGO_BIN_EXE_crab"))
                .env("HOME", &home)
                .args([command, "--json"])
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "crab {command}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            String::from_utf8(output.stdout).unwrap()
        })
    })
    .await
    .unwrap();
    server.abort();

    let plans: Value = serde_json::from_str(&plans_json).unwrap();
    assert_eq!(plans[0]["usage_micros"], 1_000_000);
    assert_eq!(plans[1]["usage_micros"], 5_000_000);

    let rates: Value = serde_json::from_str(&rates_json).unwrap();
    assert_eq!(rates["included_cpu"], 100);
    assert_eq!(rates["rates"], list);
    assert!(rates.get("plans").is_none());
}
