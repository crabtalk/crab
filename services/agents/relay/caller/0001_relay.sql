CREATE TABLE relay_agents (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    handle TEXT NOT NULL UNIQUE,
    wake TEXT NOT NULL UNIQUE DEFAULT (lower(hex(randomblob(16)))),
    name TEXT NOT NULL,
    platform TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    read_at INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE relay_conversations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    paused INTEGER NOT NULL DEFAULT 0,
    budget INTEGER NOT NULL,
    sent INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE relay_members (
    conversation INTEGER NOT NULL REFERENCES relay_conversations (id),
    agent INTEGER NOT NULL REFERENCES relay_agents (id),
    PRIMARY KEY (conversation, agent)
);
CREATE TABLE relay_messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    conversation INTEGER NOT NULL REFERENCES relay_conversations (id),
    sender INTEGER NOT NULL REFERENCES relay_agents (id),
    recipient INTEGER NOT NULL REFERENCES relay_agents (id),
    body TEXT NOT NULL,
    request_key TEXT NOT NULL,
    reply_to INTEGER REFERENCES relay_messages (id),
    created_at INTEGER NOT NULL,
    UNIQUE (sender, request_key)
);
CREATE INDEX relay_messages_conversation ON relay_messages (conversation, id);
CREATE INDEX relay_messages_recipient ON relay_messages (recipient, id);
