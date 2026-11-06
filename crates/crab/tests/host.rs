use anyhow::Result;
use crab_agent::{
    cmd::plans,
    host::{Dev, Node, Status},
};
use crabtalk_sdk::client::Client;
use std::{process, time::Duration};
use tokio::net::TcpListener;

#[tokio::test]
async fn local_host_answers_the_status_probe_without_deployed_services() -> Result<()> {
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
    let dev = Dev::new(
        node,
        Status {
            pid: process::id(),
            root: root.path().into(),
            since: 0,
        },
    );
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let client = Client::new(format!("http://{}", listener.local_addr()?));
    let server = tokio::spawn(async move { axum::serve(listener, dev.router()).await });
    let serving = client.health().await;
    server.abort();
    assert!(serving?);
    Ok(())
}

#[test]
fn local_plans_explain_how_to_sign_in_before_contacting_the_host() {
    let client = Client::new("http://127.0.0.1:0");
    for json in [false, true] {
        let error = plans::run(&client, json).unwrap_err();
        assert_eq!(
            error.to_string(),
            "plans require logging in to crab Cloud; run `crab login`"
        );
    }
}
