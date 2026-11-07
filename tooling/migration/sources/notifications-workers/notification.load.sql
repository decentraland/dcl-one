-- No delivery rows: imported history is never e-mailed again. Read state
-- only moves forward, so a read on the node survives a re-import.
INSERT INTO social.notifications
    (id, address, type, metadata, occurred_at, read, read_at, created_at, in_app)
SELECT id, address, type, metadata, occurred_at,
       read_at IS NOT NULL, read_at, created_at, true
FROM stage
ON CONFLICT (id) DO UPDATE
   SET type = EXCLUDED.type,
       metadata = EXCLUDED.metadata,
       occurred_at = EXCLUDED.occurred_at,
       read = social.notifications.read OR EXCLUDED.read,
       read_at = CASE WHEN social.notifications.read
                      THEN social.notifications.read_at
                      ELSE EXCLUDED.read_at END
