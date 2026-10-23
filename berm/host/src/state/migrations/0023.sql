-- What a call that failed ran with, as JSON: its tool, arguments, caller and
-- the image's digest. Null for every other call.

ALTER TABLE service_calls ADD COLUMN input TEXT;
