-- A service's meters by day, across its tools.
CREATE INDEX idx_service_meters_day ON service_meters (scope, name, day);
