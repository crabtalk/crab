-- Each item a source's feeds showed, once per feed, numbered in the order stored; and the last thread fetched under each watched item.

CREATE TABLE lake_items (
    seq INTEGER PRIMARY KEY,
    id TEXT NOT NULL,
    feed TEXT NOT NULL,
    posted INTEGER NOT NULL,
    data TEXT NOT NULL,
    UNIQUE (id, feed)
);

CREATE INDEX lake_items_feed ON lake_items (feed, seq);

CREATE TABLE lake_threads (
    id TEXT PRIMARY KEY,
    data TEXT NOT NULL,
    fetched INTEGER NOT NULL
);
