//! A database held to a size: a write past it fails, a size below what it
//! holds or under a page is refused, and a limit answers the one it replaces.

use anyhow::Result;
use berm::database::Databases;
use berm_api::{Migration, Sequence};
use std::path::PathBuf;

/// The program's own sequence of one migration.
fn own(name: &str, sql: &str) -> Sequence {
    Sequence {
        kit: String::new(),
        formerly: Vec::new(),
        migrations: vec![Migration {
            name: name.to_owned(),
            sql: sql.to_owned(),
        }],
    }
}

/// An empty directory for one test's databases.
fn dir(test: &str) -> Result<PathBuf> {
    let dir = std::env::temp_dir().join(format!("berm-size-{}-{test}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

#[tokio::test]
async fn a_write_past_the_size_fails() -> Result<()> {
    let databases = Databases::new(dir("past")?);
    let database = databases.get("p")?;
    database.limit(256 << 10).await?;
    let full = database
        .migrate(vec![own(
            "0001.sql",
            "CREATE TABLE t (x); INSERT INTO t VALUES (randomblob(1 << 20));",
        )])
        .await
        .expect_err("a megabyte does not fit in 256 KiB");
    assert!(format!("{full:#}").contains("full"), "{full:#}");
    Ok(())
}

#[tokio::test]
async fn a_size_below_what_it_holds_is_refused() -> Result<()> {
    let databases = Databases::new(dir("below")?);
    let database = databases.get("p")?;
    database
        .migrate(vec![own(
            "0001.sql",
            "CREATE TABLE t (x); INSERT INTO t VALUES (randomblob(1 << 20));",
        )])
        .await?;
    let refused = database
        .limit(256 << 10)
        .await
        .expect_err("it holds a megabyte");
    assert!(format!("{refused:#}").contains("more than"), "{refused:#}");
    database.limit(4 << 20).await?;
    Ok(())
}

#[tokio::test]
async fn a_size_under_a_page_is_refused() -> Result<()> {
    let databases = Databases::new(dir("page")?);
    let database = databases.get("p")?;
    let refused = database
        .limit(1)
        .await
        .expect_err("one byte is under a page");
    assert!(format!("{refused:#}").contains("page"), "{refused:#}");
    Ok(())
}

#[tokio::test]
async fn a_limit_answers_the_size_it_replaces() -> Result<()> {
    let databases = Databases::new(dir("replaces")?);
    let database = databases.get("p")?;
    database.limit(256 << 10).await?;
    assert_eq!(database.limit(4 << 20).await?, 256 << 10);
    Ok(())
}
