-- The value each service holds for a var of its `[vars]`; a var with no row
-- reads its default.
CREATE TABLE service_vars (
    scope TEXT NOT NULL,
    name TEXT NOT NULL,
    var TEXT NOT NULL,
    value TEXT NOT NULL,
    updated_at INTEGER NOT NULL DEFAULT (unixepoch()),
    PRIMARY KEY (scope, name, var),
    FOREIGN KEY (scope, name) REFERENCES services (scope, name) ON DELETE CASCADE
) WITHOUT ROWID;
