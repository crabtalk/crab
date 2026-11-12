use anyhow::Result;
use rusqlite::Connection;

#[test]
fn existing_profiles_get_unique_handles_without_losing_history_or_delivery_setup() -> Result<()> {
    let db = Connection::open_in_memory()?;
    db.execute_batch(include_str!("../migrations/0001_relay.sql"))?;
    db.execute_batch(include_str!("../migrations/0002_agent_keys.sql"))?;
    db.execute_batch(include_str!("../migrations/0003_delivery.sql"))?;
    db.execute_batch(r#"
        INSERT INTO relay_agents (owner, name, platform) VALUES
          ('alice', 'Grok Bot', '{"grok":{"webhook_url":"https://bots.example/routine","secret_id":"secret_existing"}}'),
          ('alice', 'Grok Bot', '{}'), ('alice', '123', '{}'), ('bob', 'Grok Bot', '{}'),
          ('alice', '999999999999999999999999999999999999999999999999letter', '{}');
        INSERT INTO relay_agent_keys (agent, key_id) VALUES (1, 'grok-key');
        INSERT INTO relay_conversations (owner, budget) VALUES ('alice', 100);
        INSERT INTO relay_members (conversation, agent) VALUES (1, 1), (1, 2);
        INSERT INTO relay_messages (conversation, sender, recipient, body, request_key, created_at) VALUES (1, 2, 1, 'Hello', 'first', 0);
        INSERT INTO relay_deliveries (message, next_attempt) VALUES (1, 0);
    "#)?;
    db.execute_batch(include_str!("../migrations/0004_handles.sql"))?;
    let mut statement = db.prepare("SELECT handle FROM relay_agents ORDER BY id")?;
    let handles = statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(handles[0], "grok-bot");
    assert!(handles[1].starts_with("grok-bot-"));
    assert_eq!(handles[2], "agent");
    assert_eq!(handles[3], "grok-bot");
    assert!(handles[4].starts_with("agent-"));
    let (recipient, credential, key): (String, String, String) = db.query_row(
        "SELECT a.handle, a.platform, k.key_id FROM relay_deliveries d JOIN relay_messages m ON m.id = d.message JOIN relay_agents a ON a.id = m.recipient JOIN relay_agent_keys k ON k.agent = a.id", [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?;
    assert_eq!(recipient, "grok-bot");
    assert!(credential.contains("secret_existing"));
    assert_eq!(key, "grok-key");
    Ok(())
}
