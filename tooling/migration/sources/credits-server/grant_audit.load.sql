-- Attach metadata to ledger rows already imported. The link, audit row and
-- batch checkpoint commit together; an existing link makes a replay a no-op.
WITH pending AS (
    SELECT s.*, l.id AS ledger_id, studio.name AS studio_name
    FROM stage s
    JOIN credits.ledger l ON l.tx_ref = 'credits-server:credit:' || s.credit_id
                         AND l.address = s.address
    LEFT JOIN credits.studio_budget_entries e ON e.ledger_id = l.id
    LEFT JOIN credits.studios studio ON studio.id = e.studio_id
    WHERE l.kind = 'grant' AND l.bucket = 'paid' AND l.admin_audit_id IS NULL
    FOR UPDATE OF l
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
