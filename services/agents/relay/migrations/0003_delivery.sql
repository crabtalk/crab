ALTER TABLE relay_agents ADD COLUMN generation INTEGER NOT NULL DEFAULT 1;
CREATE TABLE relay_deliveries (
    message INTEGER PRIMARY KEY REFERENCES relay_messages(id),
    generation INTEGER NOT NULL,
    state TEXT NOT NULL DEFAULT 'pending',
    attempts INTEGER NOT NULL DEFAULT 0,
    next_attempt INTEGER NOT NULL,
    status INTEGER,
    reason TEXT
);
CREATE INDEX relay_deliveries_due ON relay_deliveries(state, next_attempt, message);
