-- Each session by when it expires.

CREATE INDEX auth_sessions_expires ON auth_sessions (expires);
