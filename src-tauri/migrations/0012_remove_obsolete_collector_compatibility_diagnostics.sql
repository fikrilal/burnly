-- Burnly 0.1.28 persisted warnings for two collector compatibility bugs that
-- are repaired by the next release. Remove only those exact historical
-- failures so diagnostics health can recover after a successful refresh.

DELETE FROM diagnostic_events
WHERE code = 'antigravity.full_reconciliation_incomplete'
   OR (
        code = 'opencode.collection_failed'
        AND CASE WHEN json_valid(context_json)
            THEN json_extract(context_json, '$.failureCode') END
            = 'collector.incompatible_envelope'
        AND CASE WHEN json_valid(context_json)
            THEN json_extract(context_json, '$.source') END = 'opencode'
   )
   OR (
        code = 'collection.target_failed'
        AND CASE WHEN json_valid(context_json)
            THEN json_extract(context_json, '$.failureCode') END
            = 'collector.incompatible_envelope'
        AND CASE WHEN json_valid(context_json)
            THEN json_extract(context_json, '$.source') END
            IN ('antigravity', 'opencode')
   );
