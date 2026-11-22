CREATE TABLE relay_wakes (
    id TEXT PRIMARY KEY,
    handle TEXT NOT NULL,
    url TEXT NOT NULL,
    secret TEXT NOT NULL,
    pending INTEGER NOT NULL DEFAULT 1,
    failing INTEGER NOT NULL DEFAULT 0,
    attempts INTEGER NOT NULL DEFAULT 0,
    next_attempt INTEGER NOT NULL,
    status INTEGER,
    reason TEXT
);
CREATE INDEX relay_wakes_due ON relay_wakes(next_attempt, id) WHERE pending AND NOT failing;
