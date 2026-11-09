use axum::{
    Json, Router,
    body::Bytes,
    http::{Method, StatusCode, Uri},
};
use clap::Parser;
use crab_agent::cmd::deploy::{self, Deploy};
use crabtalk_sdk::client::{Client, Owner};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    path::Path,
    sync::{Arc, Mutex},
};
use tokio::task::JoinHandle;

const DIGEST: &str = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const MANIFEST: &str = "image = \"alice/weather:1.2.3\"\n";

#[derive(Parser)]
struct Command {
    #[command(flatten)]
    deploy: Deploy,
}

struct Registry {
    client: Client,
    requests: Arc<Mutex<Vec<(Method, String, Value)>>>,
    server: JoinHandle<()>,
}

impl Registry {
    async fn new(template_status: StatusCode) -> Self {
        let requests = Arc::new(Mutex::new(Vec::new()));
        let captured = requests.clone();
        let app = Router::new().fallback(move |method: Method, uri: Uri, body: Bytes| {
            let captured = captured.clone();
            async move {
                let body: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
                captured.lock().unwrap().push((method.clone(), uri.to_string(), body.clone()));
                let path = uri.path();
                let response = if path.starts_with("/services/connect/osv") {
                    if template_status != StatusCode::OK {
                        return (StatusCode::FORBIDDEN, Json(json!({"error":"key is not bound to that account", "explain":"E0028"})));
                    }
                    if path.ends_with("/image") { json!({"upload":null}) }
                    else { return (StatusCode::NOT_FOUND, Json(json!({"error":"unexpected request"}))); }
                } else if path == "/services/connect" {
                    json!({"osv":{"digest":body["services"]["osv"]["reference"], "published":true}})
                } else if path.starts_with("/services/alice/") && path.ends_with("/template") {
                    if template_status != StatusCode::OK {
                        return (template_status, Json(json!({"error":"this release's image is no longer retained", "explain":"E0040"})));
                    }
                    json!({"owner":"alice-id", "name":path.split('/').nth(3).unwrap(), "digest":DIGEST,
                        "manifest":{"abi_version":1, "version":"1.2.3", "tools":[]}})
                } else if path.ends_with("/runtime") {
                    json!({"tools":[], "deps":{}, "events":[]})
                } else if path.ends_with("/terms") {
                    json!({"tools":{}, "deadline":1000})
                } else if path.ends_with("/secrets") {
                    json!([])
                } else if method == Method::PUT && path.starts_with("/services/bob/") {
                    json!({"service":{"name":path.rsplit('/').next().unwrap(), "digest":DIGEST, "tools":[]}})
                } else if method == Method::POST && path == "/users/bob/services/deploys" {
                    Value::Object(body["services"].as_object().unwrap().keys().map(|name| {
                        (name.clone(), json!({"service":{"name":name, "digest":DIGEST, "tools":[]}}))
                    }).collect())
                } else if path == "/requests" || path.starts_with("/requests/") {
                    json!({"code":"TEST-CODE", "url":"https://example.com/deploy/TEST-CODE", "kind":"deploy", "status":"declined", "context":"", "created_at":0, "expires_at":900})
                } else {
                    return (StatusCode::NOT_FOUND, Json(json!({"error":"unexpected request"})));
                };
                (StatusCode::OK, Json(response))
            }
        });
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let client = Client::new(format!("http://{}", listener.local_addr().unwrap()))
            .owner(Owner::User("bob".to_owned()));
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        Self {
            client,
            requests,
            server,
        }
    }

    async fn deploy(
        &self,
        dir: &Path,
        args: &[&str],
        stopped: bool,
        yes: bool,
    ) -> anyhow::Result<()> {
        let deploy = Command::parse_from(
            ["deploy".as_ref(), dir.as_os_str()]
                .into_iter()
                .chain(args.iter().map(AsRef::as_ref)),
        )
        .deploy;
        let client = self.client.clone();
        tokio::task::spawn_blocking(move || {
            deploy::run(
                &client,
                &deploy,
                stopped,
                yes,
                Some("unused-registry"),
                false,
            )
        })
        .await
        .unwrap()
    }

    fn sent(&self, method: Method, path: &str) -> Value {
        self.requests
            .lock()
            .unwrap()
            .iter()
            .find(|(verb, url, _)| *verb == method && url == path)
            .unwrap()
            .2
            .clone()
    }
}

impl Drop for Registry {
    fn drop(&mut self) {
        self.server.abort();
    }
}

#[tokio::test]
async fn a_manifest_alone_deploys_an_exact_private_template_without_cargo() {
    let registry = Registry::new(StatusCode::OK).await;
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("crab.toml"), format!("{MANIFEST}public = true\ntemplate = true\n[database]\nseed = \"missing.db\"\nmigrations = \"missing\"\n[tools.unbuilt]\naccess = \"public\"\nprice = 100\n")).unwrap();
    registry.deploy(dir.path(), &[], true, true).await.unwrap();
    let request = registry.sent(Method::PUT, "/services/bob/weather");
    assert_eq!(request["reference"], DIGEST);
    assert_eq!(
        request["source"],
        json!({"owner":"alice-id", "name":"weather"})
    );
    assert_eq!(request["public"], false);
    assert_eq!(request["template"], false);
    assert_eq!(request["auto_upgrade"], false);
    assert_eq!(request["create"], true);
    assert_eq!(request["stopped"], true);
    for field in ["prices", "seed", "size"] {
        assert!(request.get(field).is_none(), "{request}");
    }
    registry.sent(
        Method::GET,
        "/services/alice/weather/template?version=1.2.3",
    );
}

#[tokio::test]
async fn registry_refusals_keep_their_status_and_explanation_without_building() {
    let registry = Registry::new(StatusCode::GONE).await;
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("crab.toml"), MANIFEST).unwrap();

    let error = registry
        .deploy(dir.path(), &[], false, true)
        .await
        .unwrap_err();
    let message = error.to_string();
    assert!(
        message.contains("410") && message.contains("E0040"),
        "{message}"
    );
    assert_eq!(registry.requests.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn template_manifests_use_the_cloud_approval_flow() {
    let registry = Registry::new(StatusCode::OK).await;
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("crab.toml"), MANIFEST).unwrap();
    let error = registry
        .deploy(
            dir.path(),
            &["--copy", "weather:callback=https://example.com"],
            false,
            false,
        )
        .await
        .unwrap_err();
    assert!(error.to_string().contains("declined"), "{error:#}");
    let request = registry.sent(Method::POST, "/requests");
    assert_eq!(request["kind"], "deploy");
    assert_eq!(request["services"]["weather"]["reference"], DIGEST);
    assert_eq!(
        request["copy"]["weather"]["callback"],
        "https://example.com"
    );
    assert!(
        registry
            .requests
            .lock()
            .unwrap()
            .iter()
            .all(|(method, _, _)| *method != Method::PUT)
    );
}

#[tokio::test]
async fn explicit_from_keeps_its_version_and_upgrade_controls() {
    let registry = Registry::new(StatusCode::OK).await;
    let dir = tempfile::tempdir().unwrap();
    registry
        .deploy(
            dir.path(),
            &[
                "--from",
                "alice/weather",
                "--version",
                "1.2.3",
                "--auto-upgrade",
            ],
            false,
            true,
        )
        .await
        .unwrap();
    let request = registry.sent(Method::PUT, "/services/bob/weather");
    assert_eq!(request["reference"], DIGEST);
    assert_eq!(request["auto_upgrade"], true);
}

#[tokio::test]
async fn workspace_template_members_deploy_together_under_their_local_names() {
    let registry = Registry::new(StatusCode::OK).await;
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("Cargo.toml"),
        "[workspace]\nmembers = [\"one\", \"two\"]\nresolver = \"3\"\n",
    )
    .unwrap();
    for name in ["one", "two"] {
        let member = dir.path().join(name);
        fs::create_dir_all(member.join("src")).unwrap();
        fs::write(member.join("src/lib.rs"), "").unwrap();
        fs::write(
            member.join("Cargo.toml"),
            format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\n"),
        )
        .unwrap();
        fs::write(
            member.join("crab.toml"),
            MANIFEST.replace("alice/weather", &format!("alice/{name}")),
        )
        .unwrap();
    }
    registry.deploy(dir.path(), &[], false, true).await.unwrap();
    let request = registry.sent(Method::POST, "/users/bob/services/deploys");
    let services: BTreeMap<String, Value> =
        serde_json::from_value(request["services"].clone()).unwrap();
    assert_eq!(
        services.keys().map(String::as_str).collect::<Vec<_>>(),
        ["one", "two"]
    );
    assert_eq!(request["partial"], false);
    for service in services.values() {
        assert_eq!(service["reference"], DIGEST);
        assert_eq!(
            service["project"],
            dir.path().file_name().unwrap().to_str().unwrap()
        );
    }
}

#[test]
fn build_conflicts_with_an_explicit_template_source() {
    assert!(Command::try_parse_from(["deploy", "--build", "--from", "alice/weather"]).is_err());
}

#[tokio::test]
async fn a_repo_uses_its_inherited_cargo_version_without_building() {
    let registry = Registry::new(StatusCode::OK).await;
    let root = tempfile::tempdir().unwrap();
    fs::write(
        root.path().join("Cargo.toml"),
        "[workspace]\nmembers = [\"service\"]\n[workspace.package]\nversion = \"1.2.3\"\n",
    )
    .unwrap();
    let dir = root.path().join("service");
    fs::create_dir_all(dir.join("src")).unwrap();
    fs::write(dir.join("src/lib.rs"), "").unwrap();
    fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"source-weather\"\nversion.workspace = true\n",
    )
    .unwrap();
    fs::write(dir.join("crab.toml"), "image = \"alice/weather\"\n").unwrap();
    registry.deploy(&dir, &[], false, true).await.unwrap();
    registry.sent(
        Method::GET,
        "/services/alice/weather/template?version=1.2.3",
    );
    assert!(!root.path().join("target").exists());
}

#[tokio::test]
async fn a_standalone_image_requires_a_version() {
    let registry = Registry::new(StatusCode::OK).await;
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("crab.toml"), "image = \"alice/weather\"\n").unwrap();
    let error = registry
        .deploy(dir.path(), &[], false, true)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("exact :version"), "{error:#}");
    assert!(registry.requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn publication_targets_the_image_account_and_preserves_permission_refusals() {
    for status in [StatusCode::OK, StatusCode::FORBIDDEN] {
        let registry = Registry::new(status).await;
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../services/connect/osv");
        let deploy = Command::parse_from(["publish".as_ref(), dir.as_os_str()]).deploy;
        let client = registry.client.clone();
        let result = tokio::task::spawn_blocking(move || {
            deploy::run(&client, &deploy, false, true, None, true)
        })
        .await
        .unwrap();
        registry.sent(Method::POST, "/services/connect/osv/image");
        if status == StatusCode::OK {
            result.unwrap();
            let publication = registry.sent(Method::POST, "/services/connect");
            assert_eq!(publication["services"]["osv"]["publish_only"], true);
            assert_eq!(publication["services"]["osv"]["template"], true);
        } else {
            let message = result.unwrap_err().to_string();
            assert!(
                message.contains("403") && message.contains("E0028"),
                "{message}"
            );
        }
        assert!(
            registry
                .requests
                .lock()
                .unwrap()
                .iter()
                .all(|(_, path, _)| !path.contains("/bob/"))
        );
    }
}
