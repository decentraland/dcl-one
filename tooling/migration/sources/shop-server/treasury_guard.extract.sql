-- Refill intents upstream has not resolved: a swap or transfer that may be
-- on chain without its ledger row yet. Upstream does not persist the signed
-- swap before 'swapped', so the node cannot recover a 'pending' one safely.
SELECT p.id, p.status
FROM treasury_pending_refill p
WHERE p.status NOT IN ('settled', 'aborted')
