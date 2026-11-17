-- Codes never expire upstream or in the node, so in-flight confirmation
-- links keep working. Upstream does not store the source: 'account'.
-- A newer pending request made on the node wins. Rows this load wrote are
-- recorded with their code, for unconfirmed_email_reconcile.
WITH upserted AS (
    INSERT INTO social.unconfirmed_emails (address, email, code, source, created_at)
    SELECT DISTINCT ON (address) address, email, code, 'account', updated_at
    FROM stage
    ORDER BY address, updated_at DESC, source_address
    ON CONFLICT (address) DO UPDATE
       SET email = EXCLUDED.email,
           code = EXCLUDED.code,
           source = EXCLUDED.source,
           created_at = EXCLUDED.created_at
     WHERE social.unconfirmed_emails.created_at <= EXCLUDED.created_at
    RETURNING address, code
)
INSERT INTO dcl_migration.unconfirmed_email_loaded (address, code)
SELECT address, code FROM upserted
ON CONFLICT (address) DO UPDATE SET code = EXCLUDED.code
