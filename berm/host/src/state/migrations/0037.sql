-- `tape_bytes` is the bytes of `tape` over the service's `service_replays` rows.
ALTER TABLE services ADD COLUMN tape_bytes INTEGER NOT NULL DEFAULT 0;
UPDATE services SET tape_bytes = (
    SELECT COALESCE(SUM(LENGTH(tape)), 0)
      FROM service_replays r
     WHERE r.scope = services.scope AND r.name = services.name);
