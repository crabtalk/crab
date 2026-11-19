CREATE TABLE audits (
    account TEXT NOT NULL,
    user TEXT NOT NULL,
    site TEXT NOT NULL,
    snapshot TEXT NOT NULL,
    PRIMARY KEY (account, user, site)
) WITHOUT ROWID;
