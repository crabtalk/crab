-- Each version of a record a feed of records showed, numbered in the order stored: its values from `since` until the next version of the same id in the same feed.

CREATE TABLE lake_records (
    seq INTEGER PRIMARY KEY,
    id TEXT NOT NULL,
    feed TEXT NOT NULL,
    since INTEGER NOT NULL,
    data TEXT NOT NULL
);

CREATE INDEX lake_records_id ON lake_records (id, feed, seq);
