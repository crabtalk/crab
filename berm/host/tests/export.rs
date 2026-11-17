use anyhow::Result;
use axum::{
    Extension,
    body::to_bytes,
    http::{Request, StatusCode},
    response::IntoResponse,
};
use berm_api::host;
use berm_build::fixture;
use berm_host::{Named, test::TestHost};
use rusqlite::Connection;
use serde_json::{Value, json};
use tower::ServiceExt;

#[tokio::test]
async fn kv_exports_are_isolated_and_restore_into_the_host() -> Result<()> {
    let image = fixture::image("berm-fixture")?;
    let host = TestHost::open(0).await?;
    host.deploy("alice/data", &image).await?;
    host.deploy("bob/data", &image).await?;
    for (service, text) in [("alice/data", "世界\nexport"), ("bob/data", "private")] {
        host.call_json(service, "stored_text", json!({"key":"kept","text":text}))
            .await?;
    }
    host.call_json(
        "alice/data",
        "stored_text",
        json!({"key":"empty","text":""}),
    )
    .await?;
    host.host.stop("alice/data").await?;
    let export = host.host.export("alice/data").await?;
    let conn = Connection::open(export.path())?;
    let tables: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_schema WHERE type='table'")?
        .query_map([], |row| row.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    assert_eq!(tables, ["_berm_state"]);
    assert_eq!(
        conn.query_row("PRAGMA quick_check", [], |row| row.get::<_, String>(0))?,
        "ok"
    );
    assert_eq!(
        conn.query_row(
            "SELECT value FROM _berm_state WHERE key='kept'",
            [],
            |row| row.get::<_, Vec<u8>>(0)
        )?,
        "世界\nexport".as_bytes()
    );
    drop(conn);
    host.seeded("alice/restored", &image, Some(export.path()))
        .await?;
    assert!(!host.host.databases.path("alice/restored").exists());
    assert_eq!(
        host.call_json("alice/restored", "stored_text", json!({"key":"kept"}))
            .await?["text"],
        "世界\nexport"
    );
    assert_eq!(
        host.call_json("alice/restored", "stored_text", json!({"key":"empty"}))
            .await?,
        json!({"text":"","bytes":[]})
    );
    Ok(())
}

#[tokio::test]
async fn sql_export_is_a_committed_snapshot_and_a_database_seed() -> Result<()> {
    let image = fixture::image("berm-keeper")?;
    let host = TestHost::open(0).await?;
    host.deploy("keeper", &image).await?;
    host.call("keeper", "put", br#"{"word":"committed"}"#)
        .await?;
    let writer = Connection::open(host.host.databases.path("keeper"))?;
    writer.execute_batch("BEGIN IMMEDIATE; INSERT INTO words(word) VALUES('uncommitted');")?;
    let export = host.host.export("keeper").await?;
    let conn = Connection::open(export.path())?;
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM words", [], |row| row.get::<_, i64>(0))?,
        1
    );
    assert_eq!(
        conn.query_row(
            "SELECT value FROM _berm_state WHERE key='last'",
            [],
            |row| row.get::<_, Vec<u8>>(0)
        )?,
        b"committed"
    );
    writer.execute_batch("ROLLBACK")?;
    drop(conn);
    host.seeded("restored", &image, Some(export.path())).await?;
    assert_eq!(host.call_json("restored", "count", json!({})).await?, 1);
    let copy = host.host.export("restored").await?;
    assert_eq!(
        Connection::open(copy.path())?.query_row(
            "SELECT value FROM _berm_state WHERE key='last'",
            [],
            |row| row.get::<_, Vec<u8>>(0)
        )?,
        b"committed"
    );
    Ok(())
}

#[tokio::test]
async fn export_http_streams_valid_empty_sqlite_and_explains_missing_data() -> Result<()> {
    let host = TestHost::open(0).await?;
    host.deploy("empty", &fixture::image("berm-fixture")?)
        .await?;
    let app = berm_host::router(host.host.clone()).layer(Extension(Named("empty".into())));
    let response = app
        .oneshot(Request::get(host::EXPORT).body(axum::body::Body::empty())?)
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-type"], host::SQLITE);
    assert_eq!(response.headers()["cache-control"], "private, no-store");
    assert_eq!(
        response.headers()["content-disposition"],
        "attachment; filename=\"empty.sqlite\""
    );
    let expected = response.headers()["content-length"]
        .to_str()?
        .parse::<usize>()?;
    let bytes = to_bytes(response.into_body(), usize::MAX).await?;
    assert_eq!(bytes.len(), expected);
    let file = tempfile::NamedTempFile::new()?;
    std::fs::write(file.path(), bytes)?;
    assert_eq!(
        Connection::open(file.path())?.query_row(
            "SELECT COUNT(*) FROM _berm_state",
            [],
            |row| row.get::<_, i64>(0)
        )?,
        0
    );
    host.host.remove("empty").await?;
    let response = berm_host::router(host.host.clone())
        .layer(Extension(Named("empty".into())))
        .oneshot(Request::get(host::EXPORT).body(axum::body::Body::empty())?)
        .await?;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body: Value = serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await?)?;
    assert_eq!(body["explain"], "E0079");
    Ok(())
}

#[tokio::test]
async fn seeds_refuse_existing_services_and_oversized_files_with_explanations() -> Result<()> {
    let image = fixture::image("berm-fixture")?;
    let host = TestHost::open(0).await?;
    host.deploy("original", &image).await?;
    let export = host.host.export("original").await?;
    let denied = host
        .seeded("original", &image, Some(export.path()))
        .await
        .err()
        .unwrap()
        .downcast::<berm_host::Denied>()?
        .into_response();
    assert_eq!(denied.status(), StatusCode::CONFLICT);
    let body: Value = serde_json::from_slice(&to_bytes(denied.into_body(), usize::MAX).await?)?;
    assert_eq!(body["explain"], "E0080");
    export.as_file().set_len(host::DATABASE_SIZE + 1)?;
    let denied = host
        .seeded("oversized", &image, Some(export.path()))
        .await
        .err()
        .unwrap()
        .downcast::<berm_host::Denied>()?
        .into_response();
    assert_eq!(denied.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body: Value = serde_json::from_slice(&to_bytes(denied.into_body(), usize::MAX).await?)?;
    assert_eq!(body["explain"], "E0081");
    assert!(host.host.export("oversized").await.is_err());
    Ok(())
}

#[tokio::test]
async fn kv_seed_refuses_sql_tables_instead_of_discarding_them() -> Result<()> {
    let host = TestHost::open(0).await?;
    let seed = tempfile::NamedTempFile::new()?;
    Connection::open(seed.path())?.execute_batch("CREATE TABLE _berm_state(key TEXT PRIMARY KEY,value BLOB NOT NULL); CREATE TABLE precious(data BLOB);")?;
    let denied = host
        .seeded("kv", &fixture::image("berm-fixture")?, Some(seed.path()))
        .await
        .err()
        .unwrap()
        .downcast::<berm_host::Denied>()?
        .into_response();
    assert_eq!(denied.status(), StatusCode::BAD_REQUEST);
    assert!(host.host.export("kv").await.is_err());
    Ok(())
}

#[tokio::test]
async fn malformed_key_value_rows_are_input_errors_and_install_nothing() -> Result<()> {
    let host = TestHost::open(0).await?;
    let image = fixture::image("berm-fixture")?;
    for sql in [
        "CREATE TABLE _berm_state(key,value); INSERT INTO _berm_state VALUES('key',NULL);",
        "CREATE TABLE _berm_state(key,value); INSERT INTO _berm_state VALUES('key',X'01'),('key',X'02');",
        "CREATE VIEW _berm_state AS SELECT 'key' AS key, X'01' AS value;",
    ] {
        let seed = tempfile::NamedTempFile::new()?;
        Connection::open(seed.path())?.execute_batch(sql)?;
        let denied = host
            .seeded("malformed", &image, Some(seed.path()))
            .await
            .err()
            .unwrap()
            .downcast::<berm_host::Denied>()?
            .into_response();
        assert_eq!(denied.status(), StatusCode::BAD_REQUEST);
        assert!(host.host.export("malformed").await.is_err());
    }
    Ok(())
}
