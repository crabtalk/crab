//! A program's migrations and its kits', each kept under its own name, and a
//! database from before kits taken over with its history.

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
    let dir = std::env::temp_dir().join(format!("berm-migrations-{}-{test}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

#[tokio::test]
async fn a_kit_and_the_program_may_name_a_migration_alike() -> Result<()> {
    let databases = Databases::new(dir("alike")?);
    let database = databases.get("p")?;
    let ran = database
        .migrate(vec![
            (
                "k".to_owned(),
                vec![migration("0001.sql", "CREATE TABLE k_a (x)")],
            ),
            (
                String::new(),
                vec![migration("0001.sql", "CREATE TABLE a (x)")],
            ),
        ])
        .await?;
    assert_eq!(ran, ["k/0001.sql", "0001.sql"]);
    Ok(())
}

#[tokio::test]
async fn a_database_from_before_kits_keeps_its_history() -> Result<()> {
    let dir = dir("before")?;
    let conn = rusqlite::Connection::open(dir.join("p.sqlite"))?;
    conn.execute_batch(
        "CREATE TABLE _berm_migrations (
             name TEXT PRIMARY KEY, sql TEXT NOT NULL,
             applied_at INTEGER NOT NULL DEFAULT (unixepoch()));
         INSERT INTO _berm_migrations (name, sql) VALUES ('0001.sql', 'CREATE TABLE a (x)');
         CREATE TABLE a (x);",
    )?;
    drop(conn);

    let databases = Databases::new(dir);
    let database = databases.get("p")?;
    let own = migration("0001.sql", "CREATE TABLE a (x)");
    let ran = database
        .migrate(vec![
            (
                "k".to_owned(),
                vec![migration("0001.sql", "CREATE TABLE k_a (x)")],
            ),
            (String::new(), vec![own]),
        ])
        .await?;
    assert_eq!(ran, ["k/0001.sql"]);

    let changed = database
        .migrate(vec![(
            String::new(),
            vec![migration("0001.sql", "CREATE TABLE b (x)")],
        )])
        .await
        .expect_err("a changed migration is refused");
    assert!(
        changed
            .to_string()
            .contains("0001.sql was applied and has changed"),
        "{changed}"
    );
    Ok(())
}
