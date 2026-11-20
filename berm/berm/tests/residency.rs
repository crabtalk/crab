#![cfg(feature = "sqlite")]

use anyhow::Result;
use berm::{database::Databases, syscall::sql::Statement};
use std::{
    fs,
    process::{self, Command},
    time::{Duration, Instant},
};

#[tokio::test]
async fn idle_databases_share_bounded_connections_and_no_dedicated_threads() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let databases = Databases::new(dir.path().to_owned());
    let descriptors = || {
        fs::read_dir(if cfg!(target_os = "linux") {
            "/proc/self/fd"
        } else {
            "/dev/fd"
        })
        .map(|files| files.count())
    };
    let before = descriptors()?;
    let first = databases.get("first")?;
    first.limit(65536).await?;
    for round in 0..2 {
        for i in 0..300 {
            let database = databases.get(&format!("service-{i}"))?;
            database.limit(65536).await?;
            assert_eq!(database.usage(0).await?.limit, 65536);
        }
        let after = descriptors()?;
        println!("round {round}: open descriptors {after} (baseline {before})");
        assert!(
            after <= before + 450,
            "idle connections grew with service count: {after}"
        );
    }
    let start = Instant::now();
    assert_eq!(first.limit(131072).await?, 65536);
    println!("reopen and limit latency: {:?}", start.elapsed());
    let result = first
        .read(
            0,
            Statement {
                sql: "SELECT * FROM _berm_state".into(),
                params: "[]".into(),
                deadline: Instant::now() + Duration::from_secs(2),
            },
        )
        .await;
    assert!(result.is_err(), "reopened readers still guard host tables");
    assert_eq!(first.usage(1).await?.limit, 131072);
    if cfg!(target_os = "macos") {
        let threads = Command::new("ps")
            .args(["-M", "-p", &process::id().to_string()])
            .output()?;
        let count = String::from_utf8_lossy(&threads.stdout)
            .lines()
            .count()
            .saturating_sub(1);
        println!("threads after 301 databases: {count}");
        assert!(count < 32, "a writer thread remained for each database");
        let rss = Command::new("ps")
            .args(["-o", "rss=", "-p", &process::id().to_string()])
            .output()?;
        println!(
            "RSS KiB after two sampling rounds: {}",
            String::from_utf8_lossy(&rss.stdout).trim()
        );
    }
    drop(first);
    assert!(databases.remove("first").is_ok());
    Ok(())
}
