CREATE TABLE secret_bindings (
    scope TEXT NOT NULL,
    name TEXT NOT NULL,
    storage_id TEXT NOT NULL,
    secret TEXT NOT NULL,
    host TEXT NOT NULL,
    account TEXT NOT NULL,
    PRIMARY KEY (scope, name, storage_id, secret, host),
    FOREIGN KEY (scope, name) REFERENCES services (scope, name) ON DELETE CASCADE
);
