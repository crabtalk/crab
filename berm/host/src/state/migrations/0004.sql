CREATE TABLE service_meters (
        scope TEXT NOT NULL,
        name TEXT NOT NULL,
        day INTEGER NOT NULL,
        calls INTEGER NOT NULL DEFAULT 0,
        instructions INTEGER NOT NULL DEFAULT 0,
        memory INTEGER NOT NULL DEFAULT 0,
        read INTEGER NOT NULL DEFAULT 0,
        written INTEGER NOT NULL DEFAULT 0,
        stored INTEGER NOT NULL DEFAULT 0,
        PRIMARY KEY (scope, name, day),
        FOREIGN KEY (scope, name) REFERENCES services (scope, name) ON DELETE CASCADE
    );