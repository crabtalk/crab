use axum::{
    Router,
    body::Body,
    http::{StatusCode, Uri},
    response::IntoResponse,
};
use crab_agent::cmd::export;
use crabtalk_sdk::client::{Client, Error, Owner};
use std::sync::{Arc, Mutex};

#[tokio::test]
async fn export_streams_cloud_and_local_paths_without_overwriting_files() {
    let paths = Arc::new(Mutex::new(Vec::new()));
    let captured = paths.clone();
    let app = Router::new().fallback(move |uri: Uri| {
        captured.lock().unwrap().push(uri.path().to_owned());
        async move {
            if uri.path().contains("missing") {
                return (
                    StatusCode::NOT_FOUND,
                    axum::Json(serde_json::json!({"error":"data unavailable","explain":"E0079"})),
                )
                    .into_response();
            }
            Body::from_stream(futures_util::stream::iter([
                Ok::<_, std::io::Error>(b"SQLite ".to_vec()),
                Ok(b"format 3\0".to_vec()),
            ]))
            .into_response()
        }
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let client = Client::new(format!("http://{}", listener.local_addr().unwrap()));
    let server = tokio::spawn(async { axum::serve(listener, app).await.unwrap() });
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("backup.sqlite");
    let target = path.clone();
    let local = client.clone();
    tokio::task::spawn_blocking(move || export::run(&local, "notes", Some(&target)))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"SQLite format 3\0");
    let cloud = client.clone().owner(Owner::User("alice".into()));
    let target = path.clone();
    let failed =
        tokio::task::spawn_blocking(move || export::run(&cloud, "alice/notes", Some(&target)))
            .await
            .unwrap();
    assert!(failed.is_err());
    drop(failed);
    assert_eq!(std::fs::read(&path).unwrap(), b"SQLite format 3\0");
    assert_eq!(
        *paths.lock().unwrap(),
        [
            "/services/local/notes/export",
            "/services/alice/notes/export"
        ]
    );
    let error = client.export("missing").await.err().unwrap();
    assert!(matches!(error, Error::Api{status:404,explain:Some(code),..} if code=="E0079"));
    let target = dir.path().join("missing.sqlite");
    let output = target.clone();
    assert!(
        tokio::task::spawn_blocking(move || export::run(&client, "missing", Some(&output)))
            .await
            .unwrap()
            .is_err()
    );
    assert!(!target.exists());
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    server.abort();
}
