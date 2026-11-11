CREATE TABLE relay_agent_keys (
    agent INTEGER NOT NULL REFERENCES relay_agents(id),
    key_id TEXT NOT NULL,
    PRIMARY KEY (agent, key_id)
);
