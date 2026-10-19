DROP INDEX idx_service_calls_tool;
    CREATE INDEX idx_service_calls_caller
        ON service_calls (scope, name, tool, source, caller, id);