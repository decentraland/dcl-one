-- Classify the batch once. Keep matched rows locked until the audit and the
-- ledger link commit together, so an existing link is never overwritten.
CREATE TEMP TABLE grant_audit_candidates ON COMMIT DROP AS
SELECT s.*, l.id AS ledger_id, l.admin_audit_id
FROM stage s
LEFT JOIN LATERAL (
    SELECT l.id, l.admin_audit_id
    FROM credits.ledger l
    WHERE l.tx_ref = 'credits-server:credit:' || s.credit_id
      AND l.address = s.address AND l.kind = 'grant' AND l.bucket = 'paid'
    ORDER BY l.id LIMIT 1
    FOR UPDATE
) l ON true;

WITH pending AS (
    SELECT c.*, studio.name AS studio_name
    FROM grant_audit_candidates c
    LEFT JOIN credits.studio_budget_entries e ON e.ledger_id = c.ledger_id
    LEFT JOIN credits.studios studio ON studio.id = e.studio_id
    WHERE c.ledger_id IS NOT NULL AND c.admin_audit_id IS NULL
), audits AS (
    INSERT INTO ops.admin_audit (at, actor, domain, action, target, detail)
    SELECT granted_at AT TIME ZONE 'UTC', actor, 'credits', 'grant', address,
           jsonb_build_object('reason', reason, 'annotation', annotation, 'upstreamCreditId', credit_id)
           || CASE WHEN credit_source = 'studio_grant' THEN jsonb_build_object('studioName', studio_name)
                   ELSE '{}'::jsonb END
    FROM pending
    RETURNING id, target, detail->>'upstreamCreditId' AS credit_id
)
UPDATE credits.ledger l
SET admin_audit_id = a.id
FROM pending p
JOIN audits a ON a.credit_id = p.credit_id AND a.target = p.address
WHERE l.id = p.ledger_id;

DO $$
DECLARE
    imported bigint;
    already_linked bigint;
    missing_ids text[];
    missing_id text;
BEGIN
    SELECT count(*) FILTER (WHERE ledger_id IS NOT NULL AND admin_audit_id IS NULL),
           count(*) FILTER (WHERE admin_audit_id IS NOT NULL),
           COALESCE(array_agg(credit_id ORDER BY credit_id) FILTER (WHERE ledger_id IS NULL), ARRAY[]::text[])
    INTO imported, already_linked, missing_ids
    FROM grant_audit_candidates;

    FOREACH missing_id IN ARRAY missing_ids LOOP
        RAISE WARNING 'credits-server/grant_audit: no matching paid ledger grant for upstream credit %',
            to_json(missing_id);
    END LOOP;
    -- INFO is sent even when the dry run's client_min_messages is warning.
    RAISE INFO 'credits-server/grant_audit: imported %, skipped already linked %, skipped no matching ledger row %',
        imported, already_linked, cardinality(missing_ids);
END $$;
