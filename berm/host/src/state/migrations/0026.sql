-- A call's `cpu`, `read` and `written` are what its tool's own invocation
-- used, as its budget counts it; null for a call the host refused and for
-- calls recorded before.

ALTER TABLE service_calls ADD COLUMN cpu INTEGER;
ALTER TABLE service_calls ADD COLUMN read INTEGER;
ALTER TABLE service_calls ADD COLUMN written INTEGER;
