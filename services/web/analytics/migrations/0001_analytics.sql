CREATE TABLE claims (
    domain TEXT PRIMARY KEY,
    account TEXT NOT NULL,
    token TEXT NOT NULL,
    verified_at INTEGER
);
CREATE INDEX claims_account ON claims (account);

CREATE TABLE buffer (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    domain TEXT NOT NULL,
    visitor TEXT NOT NULL,
    session TEXT NOT NULL,
    kind TEXT NOT NULL,
    path TEXT,
    referrer TEXT,
    source TEXT,
    medium TEXT,
    campaign TEXT,
    name TEXT,
    value INTEGER,
    currency TEXT,
    at INTEGER NOT NULL
);
CREATE INDEX buffer_domain ON buffer (domain, id);
