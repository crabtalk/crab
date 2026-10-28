CREATE TABLE service_meters_new (
        scope TEXT NOT NULL,
        name TEXT NOT NULL,
        tool TEXT NOT NULL DEFAULT '',
        day INTEGER NOT NULL,
        calls INTEGER NOT NULL DEFAULT 0,
        cpu INTEGER NOT NULL DEFAULT 0,
        memory INTEGER NOT NULL DEFAULT 0,
        read INTEGER NOT NULL DEFAULT 0,
        written INTEGER NOT NULL DEFAULT 0,
        stored INTEGER NOT NULL DEFAULT 0,
        proxied INTEGER NOT NULL DEFAULT 0,
        logged INTEGER NOT NULL DEFAULT 0,
        failed INTEGER NOT NULL DEFAULT 0,
        sent INTEGER NOT NULL DEFAULT 0,
        runs INTEGER NOT NULL DEFAULT 0,
        held INTEGER NOT NULL DEFAULT 0,
        PRIMARY KEY (scope, name, tool, day),
        FOREIGN KEY (scope, name) REFERENCES services (scope, name) ON DELETE CASCADE
    );
    INSERT INTO service_meters_new
        (scope, name, day, calls, cpu, memory, read, written, stored, proxied,
         logged, failed, sent, runs, held)
        SELECT scope, name, day, calls, cpu, memory, read, written, stored, proxied,
               logged, failed, sent, runs, held
          FROM service_meters;
    DROP TABLE service_meters;
    ALTER TABLE service_meters_new RENAME TO service_meters;
