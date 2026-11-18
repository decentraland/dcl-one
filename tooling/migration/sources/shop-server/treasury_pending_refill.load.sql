-- Upstream ids kept. Statuses map one to one ('settled', 'aborted'); the
-- node-only columns (nonces, raw transactions, abort_reason) stay NULL. An
-- idempotency key the target holds under another id, or a non-positive
-- amount, stops the batch. A settled intent whose ledger row is not in the
-- target is counted (the ledger step runs first).
DO $check$ DECLARE n bigint; BEGIN
  SELECT count(*) INTO n FROM stage s
   WHERE NOT (s.usdc_to_spend > 0 AND s.mana_to_acquire > 0)
      OR EXISTS (SELECT 1 FROM shop.treasury_pending_refill p
                  WHERE p.idempotency_key = s.idempotency_key AND p.id <> s.id);
  IF n > 0 THEN RAISE EXCEPTION
    'shop-server/treasury_pending_refill: % intents have a non-positive amount or an idempotency key held under another id', n;
  ELSE NULL; END IF;

  SELECT count(*) INTO n FROM stage s
   WHERE s.status = 'settled'
     AND (s.ledger_entry_id IS NULL
          OR NOT EXISTS (SELECT 1 FROM shop.treasury_ledger l WHERE l.id = s.ledger_entry_id));
  IF n > 0 THEN RAISE NOTICE 'shop-server/treasury_pending_refill: % settled intents name no ledger entry in the target', n;
  ELSE NULL; END IF; END $check$;

INSERT INTO shop.treasury_pending_refill AS p
    (id, idempotency_key, status, usdc_to_spend, mana_to_acquire, swap_tx_hash, usdc_spent,
     mana_acquired, oracle_price, transfer_tx_hash, ledger_entry_id, created_at, updated_at)
SELECT s.id, s.idempotency_key, s.status, s.usdc_to_spend, s.mana_to_acquire, s.swap_tx_hash,
       s.usdc_spent, s.mana_acquired, s.oracle_price, s.transfer_tx_hash, s.ledger_entry_id,
       s.created_at_ms, s.updated_at_ms
FROM stage s
ON CONFLICT (id) DO UPDATE SET
    status = EXCLUDED.status, swap_tx_hash = EXCLUDED.swap_tx_hash, usdc_spent = EXCLUDED.usdc_spent,
    mana_acquired = EXCLUDED.mana_acquired, oracle_price = EXCLUDED.oracle_price,
    transfer_tx_hash = EXCLUDED.transfer_tx_hash, ledger_entry_id = EXCLUDED.ledger_entry_id,
    updated_at = EXCLUDED.updated_at
WHERE p.updated_at <= EXCLUDED.updated_at
  AND (p.status, p.swap_tx_hash, p.usdc_spent, p.mana_acquired, p.oracle_price,
       p.transfer_tx_hash, p.ledger_entry_id, p.updated_at)
      IS DISTINCT FROM
      (EXCLUDED.status, EXCLUDED.swap_tx_hash, EXCLUDED.usdc_spent, EXCLUDED.mana_acquired,
       EXCLUDED.oracle_price, EXCLUDED.transfer_tx_hash, EXCLUDED.ledger_entry_id, EXCLUDED.updated_at);
