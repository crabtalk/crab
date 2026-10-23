-- A call that failed keeps its tape beside its input: every answer it had
-- from outside the guest, in order, as berm encodes one. Null for every other
-- call, and dropped oldest first past a service's share.

ALTER TABLE service_calls ADD COLUMN tape BLOB;
