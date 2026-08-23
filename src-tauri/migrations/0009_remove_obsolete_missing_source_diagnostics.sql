-- Optional local tools are not collector failures when their storage is absent.
-- Remove warnings persisted by older releases so diagnostics health can recover
-- after upgrading to the silent-absence policy.

DELETE FROM diagnostic_events
WHERE CASE
        WHEN json_valid(context_json)
        THEN json_extract(context_json, '$.failureCode')
    END = 'collector.source_not_found'
    AND (
        (code = 'cline.collection_failed'
            AND CASE WHEN json_valid(context_json)
                THEN json_extract(context_json, '$.source') END = 'cline')
        OR (code = 'zcode.collection_failed'
            AND CASE WHEN json_valid(context_json)
                THEN json_extract(context_json, '$.source') END = 'zcode')
        OR (code = 'grok.collection_failed'
            AND CASE WHEN json_valid(context_json)
                THEN json_extract(context_json, '$.source') END = 'grok-build')
        OR (code = 'commandcode.collection_failed'
            AND CASE WHEN json_valid(context_json)
                THEN json_extract(context_json, '$.source') END = 'command-code')
        OR (code = 'zed.collection_failed'
            AND CASE WHEN json_valid(context_json)
                THEN json_extract(context_json, '$.source') END = 'zed')
    );
