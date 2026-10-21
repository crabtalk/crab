-- Each signed-in session, by the sha256 of its token as lowercase hex. The
-- token itself is never kept.

CREATE TABLE auth_sessions (
    hash TEXT PRIMARY KEY,
    account INTEGER NOT NULL REFERENCES auth_accounts (id),
    expires INTEGER NOT NULL
);

CREATE INDEX auth_sessions_account ON auth_sessions (account);
