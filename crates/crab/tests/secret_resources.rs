use anyhow::Result;
use crab_agent::host::Node;
use crabtalk_sdk::{
    account::secret::{Create, Grant},
    client::{Client, Error},
};
use std::time::Duration;
use tokio::net::TcpListener;

#[tokio::test]
async fn local_resource_api_keeps_values_private_and_rotation_keeps_the_id() -> Result<()> {
    let root = tempfile::tempdir()?;
    let node = Node::new(
        root.path().into(),
        root.path().join("secrets.toml"),
        4,
        Duration::from_millis(100),
        Duration::from_secs(60),
        None,
    )
    .await?;
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let client = Client::new(format!("http://{}", listener.local_addr()?));
    let server = tokio::spawn(async move { axum::serve(listener, node.router()).await });
    let resource = client
        .create_secret_resource(&Create {
            name: "Research Bot".into(),
            value: "private-key".into(),
        })
        .await?;
    assert!(resource.id.starts_with("secret_"));
    assert!(resource.grants.is_empty());
    client
        .rotate_secret_resource(&resource.id, "rotated-private-key")
        .await?;
    let listed = client.secret_resources().await?;
    assert_eq!(listed[0].id, resource.id);
    assert!(!serde_json::to_string(&listed)?.contains("private-key"));
    let error = client
        .grant_secret_resource(
            &resource.id,
            &Grant {
                service: "missing".into(),
                hosts: vec!["api.example.com".into()],
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(error, Error::Api { status: 404, .. }));
    client.delete_secret_resource(&resource.id).await?;
    assert!(client.secret_resources().await?.is_empty());
    assert!(matches!(
        client
            .rotate_secret_resource(&resource.id, "resurrect")
            .await
            .unwrap_err(),
        Error::Api { status: 404, .. }
    ));
    server.abort();
    Ok(())
}
