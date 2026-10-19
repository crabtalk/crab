ALTER TABLE service_wakes ADD COLUMN cron TEXT;
    ALTER TABLE service_wakes ADD COLUMN declared_cron TEXT;
    UPDATE service_wakes SET
        cron = CASE
            WHEN every = 60000 THEN '* * * * *'
            WHEN every % 60000 = 0 AND every < 3600000 AND 60 % (every / 60000) = 0
                THEN '*/' || (every / 60000) || ' * * * *'
            WHEN every = 3600000 THEN '0 * * * *'
            WHEN every % 3600000 = 0 AND every < 86400000 AND 24 % (every / 3600000) = 0
                THEN '0 */' || (every / 3600000) || ' * * *'
            WHEN every = 86400000 THEN '0 0 * * *'
        END,
        declared_cron = CASE
            WHEN declared = 60000 THEN '* * * * *'
            WHEN declared % 60000 = 0 AND declared < 3600000 AND 60 % (declared / 60000) = 0
                THEN '*/' || (declared / 60000) || ' * * * *'
            WHEN declared = 3600000 THEN '0 * * * *'
            WHEN declared % 3600000 = 0 AND declared < 86400000 AND 24 % (declared / 3600000) = 0
                THEN '0 */' || (declared / 3600000) || ' * * *'
            WHEN declared = 86400000 THEN '0 0 * * *'
        END
     WHERE slot <> '';
    UPDATE service_wakes SET due = NULL WHERE slot <> '' AND cron IS NULL;
    ALTER TABLE service_wakes DROP COLUMN every;
    ALTER TABLE service_wakes DROP COLUMN declared;
    ALTER TABLE service_wakes RENAME COLUMN declared_cron TO declared;