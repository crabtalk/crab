DROP TABLE service_callers;
    ALTER TABLE services RENAME COLUMN owner_id TO scope;
    ALTER TABLE service_state RENAME COLUMN owner_id TO scope;
    ALTER TABLE service_logs RENAME COLUMN owner_id TO scope;
    ALTER TABLE service_positions RENAME COLUMN owner_id TO scope;
    ALTER TABLE service_wakes RENAME COLUMN owner_id TO scope;
    ALTER TABLE service_usage RENAME COLUMN owner_id TO scope;
    ALTER TABLE service_deprecations RENAME COLUMN owner_id TO scope;
    ALTER TABLE routes RENAME COLUMN owner_id TO scope;