CREATE TABLE workflow_schedules (
    scope TEXT NOT NULL,
    name TEXT NOT NULL,
    workflow TEXT NOT NULL,
    revision TEXT NOT NULL,
    cron TEXT NOT NULL,
    input TEXT NOT NULL,
    due INTEGER NOT NULL,
    PRIMARY KEY (scope,name,workflow),
    FOREIGN KEY (scope,name) REFERENCES services(scope,name) ON DELETE CASCADE
);
CREATE INDEX workflow_schedule_due ON workflow_schedules(due);
