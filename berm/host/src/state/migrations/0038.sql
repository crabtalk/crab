-- The last generation of each table's buffer a flush wrote, by `batch`:
-- `usage`, `logs`, `calls` or `meters`.
CREATE TABLE flushes (
    batch TEXT PRIMARY KEY,
    generation INTEGER NOT NULL
) WITHOUT ROWID;
