CREATE TABLE inbox_deliveries (
    seq INTEGER PRIMARY KEY AUTOINCREMENT,
    source TEXT NOT NULL,
    id TEXT NOT NULL,
    payload TEXT NOT NULL,
    received_at INTEGER NOT NULL,
    acknowledged INTEGER NOT NULL DEFAULT 0,
    UNIQUE (source, id)
);
CREATE INDEX inbox_pending ON inbox_deliveries (acknowledged, seq);
CREATE INDEX inbox_received ON inbox_deliveries (received_at);
