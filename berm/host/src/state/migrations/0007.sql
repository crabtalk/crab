CREATE TABLE service_terms (
        scope TEXT NOT NULL,
        name TEXT NOT NULL,
        tool TEXT NOT NULL,
        instructions INTEGER,
        rows INTEGER,
        price_micros INTEGER NOT NULL DEFAULT 0,
        PRIMARY KEY (scope, name, tool),
        FOREIGN KEY (scope, name) REFERENCES services (scope, name) ON DELETE CASCADE
    );
    INSERT INTO service_terms (scope, name, tool, instructions, rows)
        SELECT scope, name, tool, instructions, rows FROM service_budgets;
    DROP TABLE service_budgets;