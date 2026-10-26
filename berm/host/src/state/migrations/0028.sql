CREATE TABLE service_nested_calls (
    scope TEXT NOT NULL,
    name TEXT NOT NULL,
    call INTEGER NOT NULL,
    id INTEGER NOT NULL,
    parent INTEGER NOT NULL,
    service TEXT NOT NULL,
    tool TEXT NOT NULL,
    at INTEGER NOT NULL,
    ms INTEGER NOT NULL,
    outcome TEXT NOT NULL,
    cpu INTEGER NOT NULL,
    read INTEGER NOT NULL,
    written INTEGER NOT NULL,
    PRIMARY KEY (scope, name, call, id)
);
CREATE INDEX service_nested_calls_at ON service_nested_calls (at);
