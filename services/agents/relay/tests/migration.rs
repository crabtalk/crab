use anyhow::Result;
use rusqlite::Connection;

#[test]
fn caller_data_and_shared_wakes_have_separate_schemas() -> Result<()> {
    let caller = Connection::open_in_memory()?;
    caller.execute_batch(include_str!("../caller/0001_relay.sql"))?;
    let shared = Connection::open_in_memory()?;
    shared.execute_batch(include_str!("../migrations/0001_wakes.sql"))?;
    let tables = "SELECT group_concat(name, ',') FROM (SELECT name FROM sqlite_master
                  WHERE type = 'table' AND name LIKE 'relay_%' ORDER BY name)";
    assert_eq!(
        caller.query_row(tables, [], |row| row.get::<_, String>(0))?,
        "relay_agents,relay_conversations,relay_members,relay_messages"
    );
    assert_eq!(
        shared.query_row(tables, [], |row| row.get::<_, String>(0))?,
        "relay_wakes"
    );
    Ok(())
}
