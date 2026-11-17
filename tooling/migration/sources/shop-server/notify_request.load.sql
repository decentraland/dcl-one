-- The waitlist row is identified by (address, contract_address, item_id);
-- the upstream bigserial is a surrogate no wire or row references, so the
-- node assigns its own id. Status and notified_at are copied: a 'notified'
-- row is never mailed again (the node only enqueues 'pending' rows, and its
-- sender re-checks the request is still pending). A row breaking a target
-- constraint is skipped and counted.
DO $invalid$ DECLARE n bigint; BEGIN
  SELECT count(*) INTO n FROM stage s
   WHERE NOT (lower(s.address) ~ '^0x[0-9a-f]{40}$'
          AND lower(s.contract_address) ~ '^0x[0-9a-f]{40}$'
          AND char_length(s.item_id) BETWEEN 1 AND 78
          AND char_length(s.email) BETWEEN 3 AND 254
          AND s.chain_id >= 1
          AND s.status IN ('pending', 'notified', 'unsubscribed'));
  IF n > 0 THEN RAISE NOTICE 'shop-server/notify_request: skipped % requests that break the target constraints', n;
  ELSE NULL; END IF; END $invalid$;

-- Newest updated_at wins, also against a row the node wrote; within a
-- batch, two rows equal once lowercased keep the newest.
INSERT INTO shop.item_notify_requests AS r
    (address, email, contract_address, item_id, chain_id, status, created_at, updated_at, notified_at)
SELECT DISTINCT ON (lower(s.address), lower(s.contract_address), s.item_id)
       lower(s.address), s.email, lower(s.contract_address), s.item_id, s.chain_id, s.status,
       s.created_at AT TIME ZONE 'UTC', s.updated_at AT TIME ZONE 'UTC',
       s.notified_at AT TIME ZONE 'UTC'
FROM stage s
WHERE lower(s.address) ~ '^0x[0-9a-f]{40}$'
  AND lower(s.contract_address) ~ '^0x[0-9a-f]{40}$'
  AND char_length(s.item_id) BETWEEN 1 AND 78
  AND char_length(s.email) BETWEEN 3 AND 254
  AND s.chain_id >= 1
  AND s.status IN ('pending', 'notified', 'unsubscribed')
ORDER BY lower(s.address), lower(s.contract_address), s.item_id, s.updated_at DESC, s.id DESC
ON CONFLICT (address, contract_address, item_id) DO UPDATE SET
    email = EXCLUDED.email, chain_id = EXCLUDED.chain_id, status = EXCLUDED.status,
    created_at = LEAST(r.created_at, EXCLUDED.created_at),
    updated_at = EXCLUDED.updated_at, notified_at = EXCLUDED.notified_at
WHERE r.updated_at <= EXCLUDED.updated_at
  AND (r.email, r.chain_id, r.status, r.created_at, r.updated_at, r.notified_at)
      IS DISTINCT FROM
      (EXCLUDED.email, EXCLUDED.chain_id, EXCLUDED.status, LEAST(r.created_at, EXCLUDED.created_at),
       EXCLUDED.updated_at, EXCLUDED.notified_at);
