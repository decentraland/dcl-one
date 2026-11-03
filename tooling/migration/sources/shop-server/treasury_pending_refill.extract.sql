-- Resolved intents only reach here (treasury_guard refuses the others).
-- Upstream keeps no nonces, raw transactions or abort reason.
SELECT p.id,
       p.idempotency_key,
       p.status,
       p.usdc_to_spend,
       p.mana_to_acquire,
       p.swap_tx_hash,
       p.usdc_spent,
       p.mana_acquired,
       p.oracle_price,
       p.transfer_tx_hash,
       p.ledger_entry_id,
       p.created_at AS created_at_ms,
       p.updated_at AS updated_at_ms,
       timestamptz 'epoch' + p.updated_at * interval '1 millisecond' AS updated_at
FROM treasury_pending_refill p
WHERE p.status IN ('settled', 'aborted')
