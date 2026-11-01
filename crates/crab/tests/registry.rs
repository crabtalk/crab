use crab_agent::{
    cmd::deploy::{self, Deploy},
    context::Context,
    host::Node,
};
use crabtalk_sdk::client::Client;
use std::{path::Path, time::Duration};

#[test]
fn contexts_without_a_registry_remain_direct_upload_targets() {
    let context: Context = toml::from_str("host = \"http://localhost:7777\"\n").unwrap();
    assert!(context.registry.is_none());
    let context: Context =
        toml::from_str("host = \"http://localhost:7777\"\nregistry = \"localhost:5001\"\n")
            .unwrap();
    assert_eq!(context.registry.as_deref(), Some("localhost:5001"));
}

#[tokio::test]
#[ignore = "needs TEST_REGISTRY"]
async fn a_context_registry_deploys_to_a_local_node_and_survives_restart() {
    let registry = std::env::var("TEST_REGISTRY").unwrap();
    let root = tempfile::tempdir().unwrap();
    let node = Node::new(
        root.path().into(),
        root.path().join("secrets.toml"),
        8,
        Duration::from_secs(1),
        Duration::from_secs(60),
        None,
    )
    .await
    .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let router = node.clone().router();
    let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let client = Client::new(&url);
    let build = Deploy {
        dir: Path::new(env!("CARGO_MANIFEST_DIR")).join("../../services/connect/osv"),
        packages: Vec::new(),
        registry: None,
        force: false,
        pull: false,
        from: None,
        copy: Vec::new(),
    };
    tokio::task::spawn_blocking(move || {
        deploy::run(&client, &build, false, true, Some(&registry), false)
    })
    .await
    .unwrap()
    .unwrap();
    let deployed = node.get("osv").unwrap();
    let digest = deployed.digest.clone();
    let listing = Client::new(&url).listing("osv").await.unwrap();
    assert!(listing.reference.unwrap().contains("@sha256:"));
    server.abort();
    drop(deployed);
    drop(node);
    let reopened = Node::new(
        root.path().into(),
        root.path().join("secrets.toml"),
        8,
        Duration::from_secs(1),
        Duration::from_secs(60),
        None,
    )
    .await
    .unwrap();
    let restored = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Some(service) = reopened.get("osv") {
                break service;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the node restores its cached image");
    assert_eq!(restored.digest, digest);
}
