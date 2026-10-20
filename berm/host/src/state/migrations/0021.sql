-- Bytes each service's database may hold, as its last deploy set it.

ALTER TABLE services ADD COLUMN size INTEGER NOT NULL DEFAULT 1073741824;
