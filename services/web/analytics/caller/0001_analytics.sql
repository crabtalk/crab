CREATE TABLE hits (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    buffer_id INTEGER UNIQUE,
    domain TEXT NOT NULL,
    visitor TEXT NOT NULL,
    session TEXT NOT NULL,
    kind TEXT NOT NULL,
    path TEXT,
    channel TEXT NOT NULL,
    referrer TEXT,
    source TEXT,
    medium TEXT,
    campaign TEXT,
    name TEXT,
    value INTEGER,
    currency TEXT,
    at INTEGER NOT NULL
);
CREATE INDEX hits_visitor ON hits (visitor, at);
CREATE INDEX hits_at ON hits (at);

CREATE TABLE payments (
    id TEXT PRIMARY KEY,
    customer TEXT,
    visitor TEXT,
    session TEXT,
    amount INTEGER NOT NULL,
    currency TEXT NOT NULL,
    at INTEGER NOT NULL
);
CREATE INDEX payments_customer ON payments (customer);

CREATE TABLE customers (
    id TEXT PRIMARY KEY,
    visitor TEXT NOT NULL
);

CREATE TABLE cursors (
    kind TEXT PRIMARY KEY,
    created INTEGER NOT NULL
);
