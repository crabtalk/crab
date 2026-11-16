CREATE TABLE completions (
    run TEXT NOT NULL,
    attempt TEXT NOT NULL,
    body BLOB NOT NULL,
    PRIMARY KEY (run, attempt)
);
