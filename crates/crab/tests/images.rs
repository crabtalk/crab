use axum::{
    Json, Router,
    body::Bytes,
    http::{Method, StatusCode, Uri},
};
use clap::Parser;
use crab_agent::cmd::{deploy::Deploy, push::Push, up::Up};
use crabtalk_sdk::{
    client::{Client, Owner},
    services::service::Overrides,
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};
#[derive(Parser)]
struct DeployCommand {
    #[command(flatten)]
    deploy: Deploy,
}
#[derive(Parser)]
struct UpCommand {
    #[command(flatten)]
    up: Up,
}
#[derive(Parser)]
struct PushCommand {
    #[command(flatten)]
    push: Push,
}

#[test]
fn verbs_separate_source_and_images_and_refuse_retired_flags() {
    assert!(
        DeployCommand::try_parse_from([
            "deploy",
            "alice/weather",
            "--as",
            "forecast",
            "--private",
            "--price",
            "echo=0"
        ])
        .is_ok()
    );
    assert!(
        DeployCommand::try_parse_from(["deploy", "alice/weather", "--from", "alice/weather"])
            .is_err()
    );
    assert!(UpCommand::try_parse_from(["up", ".", "--registry", "localhost:5001"]).is_err());
    assert!(PushCommand::try_parse_from(["push", "--notes", "old"]).is_err());
    assert!(
        DeployCommand::try_parse_from(["deploy", "alice/weather", "--public", "--private"])
            .is_err()
    );
}

#[tokio::test]
async fn an_exact_image_deploy_needs_no_local_project() {
    let requests = Arc::new(Mutex::new(Vec::<(Method, String, Value)>::new()));
    let captured = requests.clone();
    let app=Router::new().fallback(move|method:Method,uri:Uri,bytes:Bytes|{let captured=captured.clone();async move{
        let body=serde_json::from_slice::<Value>(&bytes).unwrap_or(Value::Null);
        captured.lock().unwrap().push((method.clone(),uri.to_string(),body));
        let path=uri.path();
        let value=if path=="/images/alice/weather" {json!({"owner":"alice","name":"weather","digest":"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","manifest":{"abi_version":1,"version":"1.2.3","tools":[],"deploy":true},"deployments":[]})}
        else if path.ends_with("/deploys"){json!([])}
        else if path.ends_with("/runtime"){json!({"tools":[],"deps":{},"events":[]})}
        else if path.ends_with("/terms"){json!({"tools":{},"deadline":1000})}
        else if path.ends_with("/secrets"){json!([])}
        else if method==Method::PUT{json!({"service":{"name":"forecast","tools":[],"digest":"aaa"}})}
        else{return(StatusCode::NOT_FOUND,Json(json!({"error":"unexpected path"})));};
        (StatusCode::OK,Json(value))
    }});
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let client = Client::new(format!("http://{}", listener.local_addr().unwrap()))
        .owner(Owner::User("bob".into()));
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let command = DeployCommand::parse_from([
        "deploy",
        "alice/weather",
        "--version",
        "1.2.3",
        "--as",
        "forecast",
        "--private",
    ])
    .deploy;
    tokio::task::spawn_blocking(move || command.run(&client, &client, true))
        .await
        .unwrap()
        .unwrap();
    let requests = requests.lock().unwrap();
    assert!(
        requests
            .iter()
            .any(|(_, path, _)| path == "/images/alice/weather?version=1.2.3")
    );
    let (_, _, body) = requests
        .iter()
        .find(|(method, path, _)| *method == Method::PUT && path == "/services/bob/forecast")
        .unwrap();
    assert_eq!(body["reference"], "alice/weather:1.2.3");
    assert_eq!(body["public"], false);
    assert!(body.get("source").is_none());
    task.abort();
}

#[test]
fn overrides_preserve_omitted_settings_and_reject_internal_prices() {
    let mut previous = Overrides {
        public: Some(false),
        prices: BTreeMap::from([("echo".into(), 1000)]),
    };
    previous.extend(&Overrides {
        public: None,
        prices: BTreeMap::from([("echo".into(), 0)]),
    });
    let mut manifest:berm_api::Manifest=serde_json::from_value(json!({"abi_version":1,"public":true,"tools":[{"name":"echo","description":"Echo","public":true,"price":2000,"parameters":{}},{"name":"hidden","description":"Hidden","public":false,"parameters":{}}]})).unwrap();
    previous.apply(&mut manifest).unwrap();
    assert!(!manifest.public);
    assert_eq!(manifest.tools[0].price, None);
    previous.prices.insert("hidden".into(), 100);
    assert!(
        previous
            .apply(&mut manifest)
            .unwrap_err()
            .contains("outside callers")
    );
}

#[test]
fn cloud_settings_are_refused_locally_before_any_network_or_build() {
    for flags in [
        vec!["--public"],
        vec!["--private"],
        vec!["--price", "echo=0"],
        vec!["--auto-upgrade"],
    ] {
        let args: Vec<_> = [vec!["deploy", "alice/weather"], flags].concat();
        let command = DeployCommand::parse_from(args).deploy;
        let client = Client::new("http://127.0.0.1:1".to_owned());
        let error = command.run(&client, &client, true).unwrap_err();
        assert!(error.to_string().contains("apply only on Cloud"), "{error}");
    }
}

#[test]
fn push_skips_unpublished_services_and_refuses_naming_one() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("Cargo.toml"),
        "[package]\nname = \"weather\"\nversion = \"0.1.0\"\nedition = \"2024\"\n[lib]\npath = \"lib.rs\"\n",
    )
    .unwrap();
    std::fs::write(dir.path().join("lib.rs"), "").unwrap();
    std::fs::write(
        dir.path().join("crab.toml"),
        "[image]\nname = \"alice/weather\"\npublish = false\n",
    )
    .unwrap();
    let client = Client::new("http://127.0.0.1:9".to_owned()).owner(Owner::User("alice".into()));
    let push = |packages: &[&str]| {
        let mut args = vec!["push".to_owned(), dir.path().display().to_string()];
        for package in packages {
            args.extend(["-p".to_owned(), (*package).to_owned()]);
        }
        PushCommand::try_parse_from(args)
            .unwrap()
            .push
            .run(&client)
            .unwrap_err()
            .to_string()
    };
    assert!(push(&[]).contains("publishes"), "{}", push(&[]));
    assert!(
        push(&["weather"]).contains("image.publish = false"),
        "{}",
        push(&["weather"])
    );
}
