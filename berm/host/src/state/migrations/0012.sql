ALTER TABLE service_logs ADD COLUMN call INTEGER;
    ALTER TABLE service_logs ADD COLUMN at INTEGER NOT NULL DEFAULT 0;
    UPDATE service_logs SET at = CAST(strftime('%s', 'now') AS INTEGER) * 1000;
    CREATE INDEX idx_service_logs_call ON service_logs (scope, name, call);
    CREATE INDEX idx_service_logs_at ON service_logs (at);
    DROP TABLE service_positions;