CREATE TABLE service_wakes_new (
        scope TEXT NOT NULL,
        name TEXT NOT NULL,
        slot TEXT NOT NULL DEFAULT '',
        program TEXT NOT NULL,
        tool TEXT NOT NULL,
        args TEXT NOT NULL,
        due INTEGER,
        armed INTEGER NOT NULL DEFAULT 1,
        failures INTEGER NOT NULL DEFAULT 0,
        every INTEGER,
        declared INTEGER,
        PRIMARY KEY (scope, name, slot),
        FOREIGN KEY (scope, name) REFERENCES services (scope, name) ON DELETE CASCADE
    );
    INSERT INTO service_wakes_new (scope, name, program, tool, args, due, armed, failures)
        SELECT scope, name, program, tool, args, due, armed, failures FROM service_wakes;
    DROP TABLE service_wakes;
    ALTER TABLE service_wakes_new RENAME TO service_wakes;
    CREATE INDEX idx_service_wakes_due ON service_wakes (due);