-- Bytes of a service's logs kept when its day was sampled.
ALTER TABLE service_meters ADD COLUMN logs INTEGER NOT NULL DEFAULT 0;
