//! A service's database started from a seed.

#![allow(clippy::err_expect)]

use anyhow::Result;
use berm_api::Output;
use berm_host::test::{self, TestHost};
use rusqlite::Connection;
use std::path::{Path, PathBuf};

/// A copy of `service`'s database on `host`, at `to`.
fn snapshot(host: &TestHost, service: &str, to: &Path) -> Result<PathBuf> {
    let _ = std::fs::remove_file(to);
    Connection::open(host.host().databases.path(service))?
        .execute("VACUUM INTO ?1", [to.to_string_lossy()])?;
    Ok(to.to_owned())
}

async fn count(host: &TestHost, service: &str) -> Result<String> {
    let ran = host.call(service, "count", b"{}").await?;
    let Output::Done(count) = ran.result else {
        panic!("count runs: {:?}", ran.result);
    };
    Ok(count)
}

#[tokio::test]
async fn a_service_starts_from_its_seed() -> Result<()> {
    let image = test::build("berm-keeper")?;
    let built = TestHost::open(0).await?;
    built.deploy("keeper", &image).await?;
    built.call("keeper", "put", br#"{"word":"hi"}"#).await?;
    let seed = snapshot(
        &built,
        "keeper",
        &std::env::temp_dir().join("berm-seed-starts.sqlite"),
    )?;

    let host = TestHost::open(0).await?;
    host.seeded("keeper", &image, Some(&seed)).await?;
    assert_eq!(count(&host, "keeper").await?, "1");
    assert!(!seed.exists());
    Ok(())
}

#[tokio::test]
async fn a_service_with_a_database_is_refused_a_seed() -> Result<()> {
    let image = test::build("berm-keeper")?;
    let host = TestHost::open(0).await?;
    host.deploy("keeper", &image).await?;
    host.call("keeper", "put", br#"{"word":"kept"}"#).await?;
    let seed = snapshot(
        &host,
        "keeper",
        &std::env::temp_dir().join("berm-seed-refused.sqlite"),
    )?;

    let error = host
        .seeded("keeper", &image, Some(&seed))
        .await
        .err()
        .expect("the seed is refused");
    assert!(format!("{error:#}").contains("has a database"), "{error:#}");
    assert_eq!(count(&host, "keeper").await?, "1");
    Ok(())
}

#[tokio::test]
async fn a_seed_the_image_does_not_migrate_is_deleted() -> Result<()> {
    let image = test::build("berm-keeper")?;
    let seed = std::env::temp_dir().join("berm-seed-changed.sqlite");
    let _ = std::fs::remove_file(&seed);
    Connection::open(&seed)?.execute_batch(
        "CREATE TABLE _berm_migrations (kit TEXT NOT NULL, name TEXT NOT NULL, sql TEXT NOT NULL,
             applied_at INTEGER NOT NULL DEFAULT (unixepoch()), PRIMARY KEY (kit, name));
         INSERT INTO _berm_migrations (kit, name, sql) VALUES ('', '0001_words', 'CREATE TABLE other (x);');",
    )?;

    let host = TestHost::open(0).await?;
    let error = host
        .seeded("keeper", &image, Some(&seed))
        .await
        .err()
        .expect("the seed is refused");
    assert!(
        format!("{error:#}").contains("migration 0001_words was applied"),
        "{error:#}"
    );
    assert!(!host.host().databases.path("keeper").exists());
    host.deploy("keeper", &image).await?;
    assert_eq!(count(&host, "keeper").await?, "0");
    Ok(())
}

#[tokio::test]
async fn a_seeded_service_is_measured_at_its_seed() -> Result<()> {
    let image = test::build("berm-keeper")?;
    let built = TestHost::open(0).await?;
    built.deploy("keeper", &image).await?;
    for word in 0..2000 {
        let put = format!(r#"{{"word":"{word:0>64}"}}"#);
        built.call("keeper", "put", put.as_bytes()).await?;
    }
    let seed = snapshot(
        &built,
        "keeper",
        &std::env::temp_dir().join("berm-seed-measured.sqlite"),
    )?;
    let size = std::fs::metadata(&seed)?.len();

    let host = TestHost::open(0).await?;
    host.seeded("keeper", &image, Some(&seed)).await?;
    let meters = host.host().meters("keeper").await?;
    let stored = meters.last().map_or(0, |day| day.stored);
    assert!(stored >= size, "{stored} stored of a {size}-byte seed");
    Ok(())
}
