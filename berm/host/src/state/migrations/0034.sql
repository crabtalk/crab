-- A failed call's input and tape move to `service_replays`, kept past its call.
-- A cron tool's last run is kept on its wake, `last_call` null for none.
-- `service_calls` takes ids by AUTOINCREMENT, `sqlite_sequence` holding the
-- largest ever written, and every log takes a never-reused `id`.

CREATE TABLE service_replays (
    scope TEXT NOT NULL,
    name TEXT NOT NULL,
    call INTEGER NOT NULL,
    at INTEGER NOT NULL,
    input TEXT,
    tape BLOB,
    PRIMARY KEY (scope, name, call),
    FOREIGN KEY (scope, name) REFERENCES services (scope, name) ON DELETE CASCADE
);
CREATE INDEX idx_service_replays_at ON service_replays (at);
INSERT INTO service_replays (scope, name, call, at, input, tape)
    SELECT scope, name, id, at, input, tape FROM service_calls
     WHERE input IS NOT NULL OR tape IS NOT NULL;

ALTER TABLE service_wakes ADD COLUMN last_call INTEGER;
ALTER TABLE service_wakes ADD COLUMN last_at INTEGER;
ALTER TABLE service_wakes ADD COLUMN last_source TEXT;
ALTER TABLE service_wakes ADD COLUMN last_caller TEXT;
ALTER TABLE service_wakes ADD COLUMN last_outcome TEXT;
ALTER TABLE service_wakes ADD COLUMN last_ms INTEGER;
ALTER TABLE service_wakes ADD COLUMN last_meter TEXT;
ALTER TABLE service_wakes ADD COLUMN last_logs INTEGER;
UPDATE service_wakes
   SET (last_call, last_at, last_source, last_caller, last_outcome, last_ms, last_meter,
        last_logs) = (
        SELECT c.id, c.at, c.source, c.caller, c.outcome, c.ms, c.meter,
               (SELECT COUNT(*) FROM service_logs l
                 WHERE l.scope = c.scope AND l.name = c.name AND l.call = c.id)
          FROM service_calls c
         WHERE c.id = (
               SELECT MAX(id) FROM service_calls
                WHERE scope = service_wakes.scope AND name = service_wakes.name
                  AND tool = service_wakes.slot AND source = 'service'
                  AND caller = IIF(service_wakes.scope = '', service_wakes.name,
                                   service_wakes.scope || '/' || service_wakes.name)))
 WHERE slot <> '';

CREATE TABLE service_calls_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    scope TEXT NOT NULL,
    name TEXT NOT NULL,
    at INTEGER NOT NULL,
    tool TEXT NOT NULL,
    source TEXT NOT NULL,
    caller TEXT NOT NULL,
    outcome TEXT NOT NULL,
    ms INTEGER NOT NULL,
    meter TEXT,
    cpu INTEGER,
    read INTEGER,
    written INTEGER,
    FOREIGN KEY (scope, name) REFERENCES services (scope, name) ON DELETE CASCADE
);
INSERT INTO service_calls_new
        (id, scope, name, at, tool, source, caller, outcome, ms, meter, cpu, read, written)
    SELECT id, scope, name, at, tool, source, caller, outcome, ms, meter, cpu, read, written
      FROM service_calls;
DROP TABLE service_calls;
ALTER TABLE service_calls_new RENAME TO service_calls;
CREATE INDEX idx_service_calls_service ON service_calls (scope, name);
CREATE INDEX idx_service_calls_at ON service_calls (at);
CREATE INDEX idx_service_calls_caller ON service_calls (scope, name, tool, source, caller, id);

CREATE TABLE service_logs_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    scope TEXT NOT NULL,
    name TEXT NOT NULL,
    seq INTEGER NOT NULL,
    topics TEXT NOT NULL,
    data TEXT NOT NULL,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    call INTEGER,
    at INTEGER NOT NULL DEFAULT 0,
    UNIQUE (scope, name, seq),
    FOREIGN KEY (scope, name) REFERENCES services (scope, name) ON DELETE CASCADE
);
INSERT INTO service_logs_new (scope, name, seq, topics, data, created_at, call, at)
    SELECT scope, name, seq, topics, data, created_at, call, at FROM service_logs
     ORDER BY at, scope, name, seq;
DROP TABLE service_logs;
ALTER TABLE service_logs_new RENAME TO service_logs;
CREATE INDEX idx_service_logs_call ON service_logs (scope, name, call);
CREATE INDEX idx_service_logs_at ON service_logs (at);

CREATE INDEX service_nested_calls_call ON service_nested_calls (call);
