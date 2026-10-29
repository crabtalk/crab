-- What each service used since its first deploy, summed over its `service_meters` days.
CREATE TABLE service_totals (
    scope TEXT NOT NULL,
    name TEXT NOT NULL,
    runs INTEGER NOT NULL DEFAULT 0,
    cpu INTEGER NOT NULL DEFAULT 0,
    read INTEGER NOT NULL DEFAULT 0,
    written INTEGER NOT NULL DEFAULT 0,
    proxied INTEGER NOT NULL DEFAULT 0,
    logged INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (scope, name),
    FOREIGN KEY (scope, name) REFERENCES services (scope, name) ON DELETE CASCADE
);
INSERT INTO service_totals (scope, name, runs, cpu, read, written, proxied, logged)
    SELECT scope, name, SUM(runs), SUM(cpu), SUM(read), SUM(written), SUM(proxied), SUM(logged)
      FROM service_meters
     GROUP BY scope, name;
