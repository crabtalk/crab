ALTER TABLE services ADD COLUMN storage_id TEXT NOT NULL DEFAULT '';
UPDATE services SET storage_id = lower(hex(randomblob(16)));
