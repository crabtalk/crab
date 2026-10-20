//! A database held to a size: a write past it fails, and a size below what it
//! holds is refused.

use anyhow::Result;
use berm::database::Databases;
use berm_api::Migration;
use std::path::PathBuf;

fn migration(name: &str, sql: &str) -> Migration {
    Migration {
        name: name.to_owned(),
        sql: sql.to_owned(),
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
        .migrate(vec![(
            String::new(),
            vec![migration(
                "0001.sql",
                "CREATE TABLE t (x); INSERT INTO t VALUES (randomblob(1 << 20));",
            )],
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
        .migrate(vec![(
            String::new(),
            vec![migration(
                "0001.sql",
                "CREATE TABLE t (x); INSERT INTO t VALUES (randomblob(1 << 20));",
            )],
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
