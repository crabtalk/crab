-- Each day's visitors to a site, overall, by page and by referring host, each visitor counted once a day.
-- `seen` holds the per-day client keys that count them; prune deletes a day's keys once the day is over.

CREATE TABLE seen (
    site TEXT NOT NULL,
    day INTEGER NOT NULL,
    key TEXT NOT NULL,
    page TEXT NOT NULL,
    source TEXT NOT NULL,
    PRIMARY KEY (site, day, key, page, source)
);

CREATE INDEX seen_source ON seen (site, day, key, source);

CREATE TABLE visitors (
    site TEXT NOT NULL,
    day INTEGER NOT NULL,
    visitors INTEGER NOT NULL,
    PRIMARY KEY (site, day)
);

CREATE TABLE pages (
    site TEXT NOT NULL,
    day INTEGER NOT NULL,
    page TEXT NOT NULL,
    visitors INTEGER NOT NULL,
    PRIMARY KEY (site, day, page)
);

CREATE TABLE sources (
    site TEXT NOT NULL,
    day INTEGER NOT NULL,
    source TEXT NOT NULL,
    visitors INTEGER NOT NULL,
    PRIMARY KEY (site, day, source)
);
