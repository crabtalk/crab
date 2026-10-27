-- CPU microseconds a service's SQL statements spent on its database's threads.
ALTER TABLE service_meters ADD COLUMN sql_cpu INTEGER NOT NULL DEFAULT 0;
