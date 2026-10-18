CREATE TABLE service_calls (
        id INTEGER PRIMARY KEY,
        scope TEXT NOT NULL,
        name TEXT NOT NULL,
        at INTEGER NOT NULL,
        tool TEXT NOT NULL,
        source TEXT NOT NULL,
        caller TEXT NOT NULL,
        outcome TEXT NOT NULL,
        ms INTEGER NOT NULL,
        meter TEXT,
        price_micros INTEGER NOT NULL,
        FOREIGN KEY (scope, name) REFERENCES services (scope, name) ON DELETE CASCADE
    );
    CREATE INDEX idx_service_calls_service ON service_calls (scope, name);
    CREATE INDEX idx_service_calls_tool ON service_calls (scope, name, tool);
    CREATE INDEX idx_service_calls_at ON service_calls (at);