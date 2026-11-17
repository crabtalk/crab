use anyhow::Result;
use berm::{bound::watchdog, syscall::sql::Statement};
use berm_build::fixture;
use berm_host::{Config, Denied, test::TestHost};
use rusqlite::Connection;
use serde_json::json;
use std::time::{Duration, Instant};

const SPIN: &str =
    "WITH RECURSIVE spin(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM spin) SELECT sum(x) FROM spin";

#[tokio::test]
async fn sql_deadlines_interrupt_readers_and_writers_and_release_transactions() -> Result<()> {
    let host = TestHost::open_with(
        0,
        Config {
            deadline: Duration::from_secs(1),
            ..Config::default()
        },
    )
    .await?;
    host.deploy("keeper", &fixture::image("berm-keeper")?)
        .await?;
    let database = host.host.databases.get("keeper")?;
    let started = Instant::now();
    let error = database
        .read(
            0,
            Statement {
                sql: SPIN.into(),
                params: "[]".into(),
                deadline: Instant::now() + Duration::from_millis(30),
            },
        )
        .await
        .err()
        .expect("read interrupted");
    assert!(error.is::<watchdog::Expired>());
    assert!(started.elapsed() < Duration::from_secs(3));
    assert_eq!(
        host.sql("keeper", "SELECT 1", json!([])).await?["rows"],
        json!([[1]])
    );
    let conn = Connection::open(host.host.databases.path("keeper"))?;
    conn.execute_batch(&format!(
        "CREATE TRIGGER spin BEFORE INSERT ON words BEGIN SELECT ({SPIN}); END"
    ))?;
    let started = Instant::now();
    let error = host
        .call("keeper", "put", br#"{"word":"cancelled"}"#)
        .await
        .err()
        .expect("write interrupted");
    let denied = Denied::from(error);
    assert_eq!(denied.status.as_u16(), 504);
    assert_eq!(denied.explain, Some(watchdog::Expired::EXPLANATION));
    assert!(started.elapsed() < Duration::from_secs(3));
    conn.busy_timeout(Duration::from_secs(3))?;
    conn.execute_batch("DROP TRIGGER spin")?;
    assert_eq!(host.call_json("keeper", "count", json!({})).await?, 0);
    host.call("keeper", "put", br#"{"word":"accepted"}"#)
        .await?;
    assert_eq!(host.call_json("keeper", "count", json!({})).await?, 1);
    Ok(())
}
