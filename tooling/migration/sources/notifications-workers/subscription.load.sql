-- Newest wins, by updated_at, against rows the node already wrote.
-- is_credits_workflow is not stored upstream: inserted false, kept on update.
INSERT INTO social.subscriptions (address, email, details, updated_at)
SELECT DISTINCT ON (address) address, email, details, updated_at
FROM stage
ORDER BY address, updated_at DESC, source_address
ON CONFLICT (address) DO UPDATE
   SET email = EXCLUDED.email,
       details = EXCLUDED.details,
       updated_at = EXCLUDED.updated_at
 WHERE social.subscriptions.updated_at <= EXCLUDED.updated_at
