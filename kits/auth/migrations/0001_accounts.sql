-- Each account, and the provider identities that sign in to it. `subject` is
-- who the identity is to its provider.

CREATE TABLE auth_accounts (
    id INTEGER PRIMARY KEY,
    created INTEGER NOT NULL
);

CREATE TABLE auth_identities (
    provider TEXT NOT NULL,
    subject TEXT NOT NULL,
    account INTEGER NOT NULL REFERENCES auth_accounts (id),
    name TEXT,
    email TEXT,
    avatar TEXT,
    PRIMARY KEY (provider, subject)
);

CREATE INDEX auth_identities_account ON auth_identities (account);
