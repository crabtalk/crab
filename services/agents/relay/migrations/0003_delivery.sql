CREATE TABLE relay_deliveries (
    message INTEGER PRIMARY KEY REFERENCES relay_messages(id),
    state TEXT NOT NULL DEFAULT 'pending',
    attempts INTEGER NOT NULL DEFAULT 0,
    next_attempt INTEGER NOT NULL,
    status INTEGER,
    reason TEXT
);
CREATE INDEX relay_deliveries_due ON relay_deliveries(state, next_attempt, message);
