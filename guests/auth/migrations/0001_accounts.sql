-- Each account, and the provider identities that sign in to it. `subject` is
-- who the identity is to its provider.

CREATE TABLE accounts (
    id INTEGER PRIMARY KEY,
    created INTEGER NOT NULL
);

CREATE TABLE identities (
    provider TEXT NOT NULL,
    subject TEXT NOT NULL,
    account INTEGER NOT NULL REFERENCES accounts (id),
    name TEXT,
    email TEXT,
    avatar TEXT,
    PRIMARY KEY (provider, subject)
);

CREATE INDEX identities_account ON identities (account);
