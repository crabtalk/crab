use anyhow::Result;
use berm::{Berm, Calls, Engine, Pool, syscall::http::Response};
use berm_api::{Output, host::Caller};
use berm_build::fixture;
use berm_host::{Config, test::TestHost};
use std::{sync::Arc, time::Duration};

const SERVICE: &str = "acct/fixture";

#[tokio::test]
async fn cold_deployments_remain_discoverable_and_callable() -> Result<()> {
    let mut host = TestHost::open(0).await?;
    host.deploy(SERVICE, &fixture::image("berm-fixture")?)
        .await?;
    host.call(SERVICE, "note", br#"{"word":"kept"}"#).await?;
    let stored = host.host.live(SERVICE).await?.stored;
    host.reopen().await?;

    assert_eq!(host.host.memory("acct").loaded, 0);
    assert_eq!(host.host.idl(SERVICE)?.name, "fixture");
    assert_eq!(host.host.list("acct").len(), 1);
    assert_eq!(host.host.live(SERVICE).await?.stored, stored);
    assert_eq!(host.host.memory("acct").loaded, 0);
    let output = host
        .call(SERVICE, "echo", br#"{"query":"reloaded"}"#)
        .await?;
    assert!(matches!(output.result, Output::Done(_)));
    assert!(host.host.memory("acct").loaded > 0);
    Ok(())
}

#[tokio::test]
async fn idle_expiration_releases_code_without_removing_the_deployment() -> Result<()> {
    let host = TestHost::open_with(
        0,
        Config {
            code_idle: Duration::from_millis(20),
            ..Config::default()
        },
    )
    .await?;
    host.deploy(SERVICE, &fixture::image("berm-fixture")?)
        .await?;
    tokio::time::timeout(Duration::from_secs(3), async {
        while host.host.memory("acct").loaded != 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await?;
    assert!(host.host.idl(SERVICE).is_ok());
    assert!(matches!(
        host.call(SERVICE, "echo", br#"{"query":"again"}"#)
            .await?
            .result,
        Output::Done(_)
    ));
    Ok(())
}

#[tokio::test]
async fn uncached_code_supports_nested_calls_and_scheduled_work_after_restart() -> Result<()> {
    let mut host = TestHost::open_with(
        0,
        Config {
            code_cache: 0,
            ..Config::default()
        },
    )
    .await?;
    let image = fixture::image("berm-fixture")?;
    host.deploy(SERVICE, &image).await?;
    host.deploy("acct/other", &image).await?;
    assert_eq!(host.host.memory("acct").loaded, 0);
    let nested = host
        .call(
            SERVICE,
            "invoke",
            br#"{"service":"other","tool":"echo","args":"{\"query\":\"nested\"}"}"#,
        )
        .await?;
    assert!(matches!(nested.result, Output::Done(_)));
    host.call(SERVICE, "tick", br#"{"delay":1}"#).await?;
    host.reopen().await?;
    host.advance(2);
    assert!(!host.fire().await?.is_empty());
    assert_eq!(host.host.memory("acct").loaded, 0);
    Ok(())
}

#[tokio::test]
async fn concurrent_cold_calls_share_one_resident_image() -> Result<()> {
    let mut host = TestHost::open(0).await?;
    host.deploy(SERVICE, &fixture::image("berm-fixture")?)
        .await?;
    let loaded = host.host.memory("acct").loaded;
    host.reopen().await?;
    let (a, b, c) = tokio::join!(
        host.call(SERVICE, "echo", br#"{"query":"a"}"#),
        host.call(SERVICE, "echo", br#"{"query":"b"}"#),
        host.call(SERVICE, "echo", br#"{"query":"c"}"#),
    );
    for output in [a?, b?, c?] {
        assert!(matches!(output.result, Output::Done(_)));
    }
    assert_eq!(host.host.memory("acct").loaded, loaded);
    Ok(())
}

#[tokio::test]
async fn a_call_keeps_its_image_and_memory_accounting_after_removal() -> Result<()> {
    let host = TestHost::open_with(
        0,
        Config {
            code_cache: 0,
            ..Config::default()
        },
    )
    .await?;
    host.deploy(SERVICE, &fixture::image("berm-fixture")?)
        .await?;
    host.fetch.respond(
        "GET",
        "http://127.0.0.1/slow",
        Response {
            status: 200,
            headers: vec![],
            body: b"done".to_vec(),
        },
        Duration::from_millis(300),
    );
    let calling = host.host.clone();
    let task = tokio::spawn(async move {
        calling
            .execute(
                SERVICE,
                "fetch",
                br#"{"url":"http://127.0.0.1/slow"}"#.to_vec(),
                None,
                None,
                Default::default(),
            )
            .await
    });
    tokio::time::timeout(Duration::from_secs(3), async {
        while host.fetch.fetched().is_empty() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await?;
    assert!(host.host.memory("acct").loaded > 0);
    assert!(host.host.memory("acct").running > 0);
    host.host.remove(SERVICE).await?;
    assert!(host.host.get(SERVICE).is_none());
    assert!(host.host.memory("acct").loaded > 0);
    assert!(matches!(task.await??.result, Output::Done(_)));
    assert_eq!(host.host.memory("acct").loaded, 0);
    assert_eq!(host.host.memory("acct").running, 0);
    Ok(())
}

#[tokio::test]
async fn memory_pressure_reclaims_idle_code_before_refusing_a_call() -> Result<()> {
    let host = TestHost::open(0).await?;
    let image = fixture::image("berm-fixture")?;
    host.deploy(SERVICE, &image).await?;
    let code = host.host.memory("acct").loaded;
    let ran = host
        .call(SERVICE, "echo", br#"{"query":"measured"}"#)
        .await?;
    host.deploy("acct/other", &image).await?;
    host.host
        .limit([("acct".to_owned(), code + ran.meter.memory)].into());
    let ran = host.call(SERVICE, "echo", br#"{"query":"fits"}"#).await?;
    assert!(matches!(ran.result, Output::Done(_)), "{:?}", ran.result);
    assert!(host.host.memory("acct").loaded <= code);
    assert_eq!(host.host.list("acct").len(), 2);
    Ok(())
}

#[tokio::test]
async fn a_cold_module_cannot_exceed_a_lowered_scope_limit() -> Result<()> {
    let mut host = TestHost::open(0).await?;
    host.deploy(SERVICE, &fixture::image("berm-fixture")?)
        .await?;
    host.reopen().await?;
    host.host.limit([("acct".to_owned(), 1)].into());
    assert!(
        host.call(SERVICE, "echo", br#"{"query":"too big"}"#)
            .await
            .is_err()
    );
    assert_eq!(host.host.memory("acct").loaded, 0);
    let caller = Caller {
        id: "acct".to_owned(),
        key: None,
        app: None,
        user: None,
        client: None,
        agent: None,
    };
    let denied = host
        .host
        .call(
            SERVICE,
            Some(&caller),
            "echo",
            br#"{"query":"refused"}"#.to_vec(),
            false,
        )
        .await
        .unwrap_err();
    assert_eq!(denied.status, 429);
    assert_eq!(denied.explain, Some("E0019"));
    assert!(host.host.idl(SERVICE).is_ok());
    host.host.limit(Default::default());
    assert!(matches!(
        host.call(SERVICE, "echo", br#"{"query":"retried"}"#)
            .await?
            .result,
        Output::Done(_)
    ));
    Ok(())
}

#[tokio::test]
async fn the_byte_budget_evicts_code_and_keeps_deployments() -> Result<()> {
    let host = TestHost::open_with(
        0,
        Config {
            code_cache: 1_000_000,
            ..Config::default()
        },
    )
    .await?;
    let image = fixture::image("berm-fixture")?;
    for name in [SERVICE, "acct/other", "acct/third"] {
        host.deploy(name, &image).await?;
    }
    tokio::time::timeout(Duration::from_secs(3), async {
        while host.host.memory("acct").loaded > 1_000_000 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await?;
    assert_eq!(host.host.list("acct").len(), 3);
    for name in [SERVICE, "acct/other", "acct/third"] {
        assert!(matches!(
            host.call(name, "echo", br#"{"query":"still here"}"#)
                .await?
                .result,
            Output::Done(_)
        ));
    }
    Ok(())
}

#[tokio::test]
async fn redeployment_keeps_an_active_generation_counted_until_it_finishes() -> Result<()> {
    let host = TestHost::open_with(
        0,
        Config {
            code_cache: 0,
            ..Config::default()
        },
    )
    .await?;
    let image = fixture::image("berm-fixture")?;
    host.deploy(SERVICE, &image).await?;
    host.fetch.respond(
        "GET",
        "http://127.0.0.1/slow",
        Response {
            status: 200,
            headers: vec![],
            body: b"done".to_vec(),
        },
        Duration::from_millis(300),
    );
    let calling = host.host.clone();
    let task = tokio::spawn(async move {
        calling
            .execute(
                SERVICE,
                "fetch",
                br#"{"url":"http://127.0.0.1/slow"}"#.to_vec(),
                None,
                None,
                Default::default(),
            )
            .await
    });
    tokio::time::timeout(Duration::from_secs(3), async {
        while host.fetch.fetched().is_empty() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await?;
    let loaded = host.host.memory("acct").loaded;
    host.deploy(SERVICE, &image).await?;
    assert_eq!(host.host.memory("acct").loaded, loaded);
    assert_eq!(host.host.memory("acct").services["fixture"].loaded, loaded);
    assert!(host.host.memory("acct").services["fixture"].running > 0);
    assert!(matches!(task.await??.result, Output::Done(_)));
    assert_eq!(host.host.memory("acct").loaded, 0);
    assert!(matches!(
        host.call(SERVICE, "echo", br#"{"query":"new"}"#)
            .await?
            .result,
        Output::Done(_)
    ));
    Ok(())
}

#[tokio::test]
async fn legacy_rows_compile_once_and_record_their_footprint() -> Result<()> {
    let mut host = TestHost::open(0).await?;
    host.deploy(SERVICE, &fixture::image("berm-fixture")?)
        .await?;
    host.host
        .store
        .run(|conn| {
            conn.execute("UPDATE services SET code_bytes = NULL", [])?;
            Ok(())
        })
        .await?;
    host.reopen().await?;
    assert_eq!(host.host.memory("acct").loaded, 0);
    let footprint: i64 = host
        .host
        .store
        .read(|conn| {
            conn.query_row(
                "SELECT code_bytes FROM services WHERE scope = 'acct' AND name = 'fixture'",
                [],
                |row| row.get(0),
            )
        })
        .await?;
    assert!(footprint > 0);
    host.reopen().await?;
    assert_eq!(host.host.memory("acct").loaded, 0);
    assert!(matches!(
        host.call(SERVICE, "echo", br#"{"query":"migrated"}"#)
            .await?
            .result,
        Output::Done(_)
    ));
    Ok(())
}

#[tokio::test]
async fn an_expired_call_does_not_load_cold_code() -> Result<()> {
    let mut host = TestHost::open_with(
        0,
        Config {
            deadline: Duration::ZERO,
            ..Config::default()
        },
    )
    .await?;
    host.deploy(SERVICE, &fixture::image("berm-fixture")?)
        .await?;
    host.reopen().await?;
    let error = host
        .call(SERVICE, "echo", br#"{"query":"expired"}"#)
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("deadline"));
    assert_eq!(host.host.memory("acct").loaded, 0);
    Ok(())
}

#[tokio::test]
async fn concurrent_calls_pin_one_image_when_retention_is_disabled() -> Result<()> {
    let mut host = TestHost::open_with(
        0,
        Config {
            code_cache: 0,
            ..Config::default()
        },
    )
    .await?;
    host.deploy(SERVICE, &fixture::image("berm-fixture")?)
        .await?;
    let bytes = host.host.get(SERVICE).unwrap().bytes;
    host.reopen().await?;
    host.fetch.respond(
        "GET",
        "http://127.0.0.1/shared",
        Response {
            status: 200,
            headers: vec![],
            body: b"done".to_vec(),
        },
        Duration::from_millis(300),
    );
    let mut tasks = Vec::new();
    for _ in 0..3 {
        let calling = host.host.clone();
        tasks.push(tokio::spawn(async move {
            calling
                .execute(
                    SERVICE,
                    "fetch",
                    br#"{"url":"http://127.0.0.1/shared"}"#.to_vec(),
                    None,
                    None,
                    Default::default(),
                )
                .await
        }));
    }
    tokio::time::timeout(Duration::from_secs(3), async {
        while host.fetch.fetched().len() != 3 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await?;
    assert_eq!(host.host.memory("acct").loaded, bytes);
    for task in tasks {
        assert!(matches!(task.await??.result, Output::Done(_)));
    }
    assert_eq!(host.host.memory("acct").loaded, 0);
    assert_eq!(host.host.memory("acct").running, 0);
    Ok(())
}

#[tokio::test]
async fn legacy_restore_keeps_metadata_when_the_scope_limit_is_too_small() -> Result<()> {
    let pool = Arc::new(Pool::default());
    pool.limit(1);
    let mut calls = Calls::by_name(0);
    let drawing = pool.clone();
    calls.pool = Arc::new(move |_| drawing.clone());
    let berm = Berm::new(&Engine::default(), calls, vec![]);
    let image = fixture::image("berm-fixture")?;
    let restored = berm.restore("fixture", &image, None)?;
    assert!(restored.bytes > 1);
    assert_eq!(pool.now().loaded, 0);
    assert_eq!(restored.idl("fixture").name, "fixture");
    let error = berm
        .call(
            None,
            "fixture",
            "echo",
            br#"{"query":"over limit"}"#.to_vec(),
        )
        .await
        .unwrap_err();
    assert!(error.is::<berm::MemoryLimit>());
    pool.limit(0);
    assert!(matches!(
        berm.call(None, "fixture", "echo", br#"{"query":"allowed"}"#.to_vec())
            .await?
            .result,
        Output::Done(_)
    ));
    Ok(())
}
