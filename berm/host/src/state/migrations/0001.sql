CREATE TABLE services (
        owner_id TEXT NOT NULL,
        name TEXT NOT NULL,
        reference TEXT NOT NULL,
        digest TEXT NOT NULL,
        version TEXT NOT NULL,
        commits INTEGER NOT NULL DEFAULT 0,
        seq INTEGER NOT NULL DEFAULT 0,
        created_at INTEGER NOT NULL DEFAULT (unixepoch()),
        updated_at INTEGER NOT NULL DEFAULT (unixepoch()),
        PRIMARY KEY (owner_id, name)
    );
    CREATE TABLE service_state (
        owner_id TEXT NOT NULL,
        name TEXT NOT NULL,
        key TEXT NOT NULL,
        value BLOB NOT NULL,
        PRIMARY KEY (owner_id, name, key),
        FOREIGN KEY (owner_id, name) REFERENCES services (owner_id, name) ON DELETE CASCADE
    );
    CREATE TABLE service_logs (
        owner_id TEXT NOT NULL,
        name TEXT NOT NULL,
        seq INTEGER NOT NULL,
        topics TEXT NOT NULL,
        data TEXT NOT NULL,
        created_at INTEGER NOT NULL DEFAULT (unixepoch()),
        PRIMARY KEY (owner_id, name, seq),
        FOREIGN KEY (owner_id, name) REFERENCES services (owner_id, name) ON DELETE CASCADE
    );
    CREATE TABLE service_positions (
        owner_id TEXT NOT NULL,
        name TEXT NOT NULL,
        program TEXT NOT NULL,
        seq INTEGER NOT NULL,
        PRIMARY KEY (owner_id, name, program),
        FOREIGN KEY (owner_id, name) REFERENCES services (owner_id, name) ON DELETE CASCADE
    );
    CREATE TABLE service_wakes (
        owner_id TEXT NOT NULL,
        name TEXT NOT NULL,
        program TEXT NOT NULL,
        tool TEXT NOT NULL,
        args TEXT NOT NULL,
        due INTEGER NOT NULL,
        armed INTEGER NOT NULL DEFAULT 1,
        PRIMARY KEY (owner_id, name),
        FOREIGN KEY (owner_id, name) REFERENCES services (owner_id, name) ON DELETE CASCADE
    );
    CREATE INDEX idx_service_wakes_due ON service_wakes (due);
    CREATE TABLE service_usage (
        owner_id TEXT NOT NULL,
        name TEXT NOT NULL,
        kind TEXT NOT NULL,
        item TEXT NOT NULL,
        source TEXT NOT NULL,
        caller TEXT NOT NULL,
        outcome TEXT NOT NULL,
        day INTEGER NOT NULL,
        calls INTEGER NOT NULL,
        last INTEGER NOT NULL,
        PRIMARY KEY (owner_id, name, kind, item, source, caller, outcome, day),
        FOREIGN KEY (owner_id, name) REFERENCES services (owner_id, name) ON DELETE CASCADE
    );
    CREATE TABLE service_deprecations (
        owner_id TEXT NOT NULL,
        name TEXT NOT NULL,
        tool TEXT NOT NULL,
        since INTEGER NOT NULL,
        sunset INTEGER,
        PRIMARY KEY (owner_id, name, tool),
        FOREIGN KEY (owner_id, name) REFERENCES services (owner_id, name) ON DELETE CASCADE
    );
    CREATE TABLE service_callers (
        owner_id TEXT NOT NULL,
        name TEXT NOT NULL,
        tool TEXT NOT NULL,
        kind TEXT NOT NULL,
        caller TEXT NOT NULL,
        PRIMARY KEY (owner_id, name, tool, kind, caller),
        FOREIGN KEY (owner_id, name) REFERENCES services (owner_id, name) ON DELETE CASCADE
    );
    CREATE TABLE routes (
        owner_id TEXT NOT NULL,
        name TEXT NOT NULL,
        node TEXT NOT NULL,
        PRIMARY KEY (owner_id, name)
    );