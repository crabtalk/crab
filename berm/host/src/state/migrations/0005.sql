CREATE TABLE service_budgets (
        scope TEXT NOT NULL,
        name TEXT NOT NULL,
        tool TEXT NOT NULL,
        instructions INTEGER NOT NULL,
        rows INTEGER NOT NULL,
        PRIMARY KEY (scope, name, tool),
        FOREIGN KEY (scope, name) REFERENCES services (scope, name) ON DELETE CASCADE
    );