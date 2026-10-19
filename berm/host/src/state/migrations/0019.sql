ALTER TABLE services ADD COLUMN log_bytes INTEGER NOT NULL DEFAULT 0;
    UPDATE services SET log_bytes = (
        SELECT COALESCE(SUM(LENGTH(CAST(topics AS BLOB)) + LENGTH(CAST(data AS BLOB))), 0)
          FROM service_logs l
         WHERE l.scope = services.scope AND l.name = services.name);