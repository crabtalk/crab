use anyhow::Result;
use axum::http::{HeaderMap, StatusCode, header};
use berm_abi::{ABI_SECTION, ABI_VERSION, SERVICE_SECTION, wire};
use berm_api::{Image, Output, host::Caller, storage};
use berm_build::fixture;
use berm_host::test::TestHost;
use serde_json::{Value, json};
use std::fmt::Write;

fn image(extra: Option<&str>) -> Result<Vec<u8>> {
    let read = wire::request(&[b"SELECT n FROM counter", b"[]"]);
    let write = wire::request(&[b"UPDATE counter SET n = n + 1 RETURNING n", b"[]"]);
    let wat = |bytes: &[u8]| {
        bytes.iter().fold(String::new(), |mut out, byte| {
            write!(&mut out, "\\{byte:02x}").unwrap();
            out
        })
    };
    let wasm = wat::parse_str(format!(
        r#"(module
            (import "berm" "berm.caller.sql" (func $sql (param i32 i32) (result i64)))
            (import "berm" "berm.result.read" (func $result (param i32 i32) (result i64)))
            (import "berm" "berm.done" (func $done (param i32 i32) (result i64)))
            (memory (export "memory") 1)
            (data (i32.const 0) "{}")
            (data (i32.const 128) "{}")
            (func $run (param $start i32) (param $len i32) (local $n i32)
                local.get $start local.get $len call $sql i32.wrap_i64 local.set $n
                i32.const 256 local.get $n call $result drop
                i32.const 256 local.get $n call $done drop)
            (func (export "berm_tool_read") i32.const 0 i32.const {} call $run)
            (func (export "berm_tool_increment") i32.const 128 i32.const {} call $run)
        )"#,
        wat(&read),
        wat(&write),
        read.len(),
        write.len()
    ))?;
    let records = [("read", true), ("increment", false)]
        .map(|(name, query)| json!({
            "tool": {"name":name,"description":"Counter","parameters":{"type":"object"},"query":query,"access":"anonymous"},
            "abi_version":ABI_VERSION,"crate":"counter"
        }).to_string())
        .concat();
    let image = Image(&wasm).with_section(ABI_SECTION, records.as_bytes())?;
    let mut migrations = vec![json!({
        "name":"001.sql","sql":"CREATE TABLE counter(n INTEGER NOT NULL); INSERT INTO counter VALUES (0);"
    })];
    if let Some(sql) = extra {
        migrations.push(json!({"name":"002.sql","sql":sql}));
    }
    Ok(Image(&image).with_section(
        SERVICE_SECTION,
        &serde_json::to_vec(&json!({
            "name":"counter","buffer":16384,"caller_storage":{"migrations":migrations}
        }))?,
    )?)
}

fn identity(account: &str) -> Caller {
    Caller {
        account: Some(account.into()),
        id: "member".into(),
        key: None,
        app: None,
        user: None,
        client: None,
        agent: None,
    }
}

async fn count(host: &TestHost, service: &str, caller: &Caller, tool: &str) -> Result<i64> {
    let (_, result) = host
        .host
        .call(service, Some(caller), tool, b"{}".to_vec(), tool == "read")
        .await?;
    let Output::Done(result) = result else {
        anyhow::bail!("{result:?}")
    };
    Ok(serde_json::from_str::<Value>(&result)?["rows"][0][0]
        .as_i64()
        .unwrap())
}

#[tokio::test]
async fn files_are_isolated_migrate_lazily_and_survive_service_removal() -> Result<()> {
    let host = TestHost::open(0).await?;
    let initial = image(None)?;
    let deployed = host.deploy("counter", &initial).await?;
    let alice = identity("alice");
    let bob = identity("bob");
    let id = deployed.storage_id.get().unwrap();
    let root = host.host.databases.root().join("callers").join(id);
    let retained = host.host.databases.root().join("retained").join(id);
    let key = storage::Identity {
        account: "alice".into(),
        user: None,
    }
    .digest();
    let filename = format!("{key}.sqlite");
    assert_eq!(count(&host, "counter", &alice, "read").await?, 0);
    assert!(!root.exists());
    let (headers, read) = host
        .host
        .call("counter", None, "read", b"{}".to_vec(), true)
        .await?;
    let Output::Done(read) = read else {
        anyhow::bail!("{read:?}")
    };
    assert_eq!(serde_json::from_str::<Value>(&read)?["rows"][0][0], 0);
    assert!(!root.exists(), "an anonymous read must not create a file");
    assert_eq!(headers[header::CACHE_CONTROL], "private, no-store");
    assert!(!headers.contains_key(header::ETAG));
    let error = host
        .host
        .outside(
            "counter",
            "increment",
            false,
            b"{}".to_vec(),
            None,
            HeaderMap::new(),
        )
        .await
        .unwrap_err();
    assert_eq!(error.status, StatusCode::FORBIDDEN);
    assert_eq!(error.explain, Some(storage::ANONYMOUS));
    let (one, two) = tokio::try_join!(
        count(&host, "counter", &alice, "increment"),
        count(&host, "counter", &alice, "increment")
    )?;
    assert_eq!(one.min(two), 1);
    assert_eq!(one.max(two), 2);
    assert_eq!(count(&host, "counter", &bob, "read").await?, 0);
    assert!(root.join(&filename).is_file());

    host.deploy("counter", &image(Some("not a migration"))?)
        .await?;
    let error = host
        .host
        .call("counter", Some(&alice), "read", b"{}".to_vec(), true)
        .await
        .unwrap_err();
    assert_eq!(error.status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(error.explain, Some(storage::MIGRATION_FAILED));
    host.flush().await?;
    let logs = host.logs("counter", Some("caller_storage")).await?;
    assert_eq!(logs.len(), 1);
    assert!(logs[0].data.contains("not a migration"), "{logs:?}");

    host.deploy("counter", &image(Some("UPDATE counter SET n = n + 10"))?)
        .await?;
    assert_eq!(count(&host, "counter", &alice, "read").await?, 12);
    assert_eq!(count(&host, "counter", &alice, "read").await?, 12);
    host.host.remove("counter").await?;
    assert!(!root.exists());
    assert!(retained.join(&filename).is_file());
    host.deploy("counter", &initial).await?;
    assert_eq!(count(&host, "counter", &alice, "read").await?, 0);
    assert!(retained.join(&filename).is_file());
    Ok(())
}

#[tokio::test]
async fn caller_files_follow_the_billed_service_and_verified_user() -> Result<()> {
    let host = TestHost::open(0).await?;
    let counter = "sender/counter";
    let relay = "sender/relay";
    host.deploy(counter, &image(None)?).await?;
    host.deploy(relay, &fixture::image("berm-fixture")?).await?;
    let mut member = identity("customer");
    member.key = Some("first-key".into());
    assert_eq!(count(&host, counter, &member, "increment").await?, 1);
    let mut second = member.clone();
    second.id = "second-member".into();
    second.key = Some("second-key".into());
    assert_eq!(count(&host, counter, &second, "read").await?, 1);
    let sender = identity("sender");
    let mut app = sender.clone();
    app.app = Some("browser".into());
    app.user = Some(serde_json::from_value(
        json!({"issuer":"https://id.example","subject":"one"}),
    )?);
    assert_eq!(count(&host, counter, &app, "increment").await?, 1);
    app.user.as_mut().unwrap().subject = "two".into();
    assert_eq!(count(&host, counter, &app, "read").await?, 0);
    assert_eq!(count(&host, counter, &sender, "read").await?, 0);

    let args = serde_json::to_vec(&json!({"service":"counter","tool":"increment","args":"{}"}))?;
    let (_, nested) = host
        .host
        .call(relay, Some(&member), "invoke", args, false)
        .await?;
    assert!(matches!(nested, Output::Done(_)), "{nested:?}");
    assert_eq!(count(&host, counter, &sender, "read").await?, 1);
    assert_eq!(count(&host, counter, &member, "read").await?, 1);
    Ok(())
}
