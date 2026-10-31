//! A service's migrations and its kits', each kept under its own key, a
//! database from before kits taken over with its history, and a kit's history
//! kept under a former key moved under its id.

use anyhow::Result;
use berm::database::Databases;
use berm_api::{Migration, Sequence};
use std::path::PathBuf;

fn migration(name: &str, sql: &str) -> Migration {
    Migration {
        name: name.to_owned(),
        sql: sql.to_owned(),
    }
}

fn sequence(kit: &str, formerly: &[&str], migrations: Vec<Migration>) -> Sequence {
    Sequence {
        kit: kit.to_owned(),
        formerly: formerly.iter().map(|key| (*key).to_owned()).collect(),
        migrations,
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
async fn a_kit_and_the_service_may_name_a_migration_alike() -> Result<()> {
    let databases = Databases::new(dir("alike")?);
    let database = databases.get("p")?;
    let ran = database
        .migrate(vec![
            sequence(
                "k",
                &[],
                vec![migration("0001.sql", "CREATE TABLE k_a (x)")],
            ),
            sequence("", &[], vec![migration("0001.sql", "CREATE TABLE a (x)")]),
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
    let ran = database
        .migrate(vec![
            sequence(
                "k",
                &[],
                vec![migration("0001.sql", "CREATE TABLE k_a (x)")],
            ),
            sequence("", &[], vec![migration("0001.sql", "CREATE TABLE a (x)")]),
        ])
        .await?;
    assert_eq!(ran, ["k/0001.sql"]);

    let changed = database
        .migrate(vec![sequence(
            "",
            &[],
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

#[tokio::test]
async fn a_kits_history_under_a_former_key_moves_under_its_id() -> Result<()> {
    let databases = Databases::new(dir("formerly")?);
    let database = databases.get("p")?;
    let first = migration("0001.sql", "CREATE TABLE k_a (x)");
    database
        .migrate(vec![sequence("crab-k", &[], vec![first.clone()])])
        .await?;

    let ran = database
        .migrate(vec![sequence(
            "0123456789abcdef0123456789abcdef",
            &["crabkit-k", "crab-k"],
            vec![first, migration("0002.sql", "ALTER TABLE k_a ADD COLUMN y")],
        )])
        .await?;
    assert_eq!(ran, ["0123456789abcdef0123456789abcdef/0002.sql"]);
    Ok(())
}

#[tokio::test]
async fn a_kits_history_under_two_of_its_keys_is_refused() -> Result<()> {
    let databases = Databases::new(dir("twice")?);
    let database = databases.get("p")?;
    database
        .migrate(vec![
            sequence(
                "crab-k",
                &[],
                vec![migration("0001.sql", "CREATE TABLE k_a (x)")],
            ),
            sequence(
                "0123456789abcdef0123456789abcdef",
                &[],
                vec![migration("0001.sql", "CREATE TABLE k_b (x)")],
            ),
        ])
        .await?;

    let refused = database
        .migrate(vec![sequence(
            "0123456789abcdef0123456789abcdef",
            &["crab-k"],
            vec![migration("0001.sql", "CREATE TABLE k_b (x)")],
        )])
        .await
        .expect_err("its history is kept under two keys");
    assert!(refused.to_string().contains("are kept under"), "{refused}");
    Ok(())
}
