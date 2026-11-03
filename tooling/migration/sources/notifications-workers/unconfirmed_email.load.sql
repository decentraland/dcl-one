-- Codes never expire upstream or in the node, so in-flight confirmation
-- links keep working. Upstream does not store the source: 'account'.
-- A newer pending request made on the node wins.
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
