CREATE TABLE identities (run TEXT PRIMARY KEY, request_hash TEXT NOT NULL);
INSERT INTO identities(run,request_hash) SELECT run,request_hash FROM admissions;
