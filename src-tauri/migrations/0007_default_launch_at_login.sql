UPDATE app_settings
SET
    launch_at_login = 1,
    updated_at_ms = CASE
        WHEN updated_at_ms < created_at_ms THEN created_at_ms
        ELSE updated_at_ms
    END
WHERE id = 1;
