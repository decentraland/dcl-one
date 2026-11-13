-- Upstream ids kept; rows are immutable, so a replay inserts nothing. Any
-- row that would not land exactly stops the batch, because a skipped or
-- altered entry breaks the ledger's sums: an unknown type, a non-integer
-- amount, an id the target holds with other content, or a (type, reference)
-- the target holds under another id.
DO $check$ DECLARE n bigint; BEGIN
  SELECT count(*) INTO n FROM stage s
   WHERE s.type NOT IN ('usdc_deposit', 'usdc_settlement', 'refill', 'fee_retained', 'withdrawal_recorded')
      OR s.usdc_delta <> trunc(s.usdc_delta) OR s.mana_delta <> trunc(s.mana_delta);
  IF n > 0 THEN RAISE EXCEPTION 'shop-server/treasury_ledger: % entries have an unknown type or a fractional amount', n;
  ELSE NULL; END IF;

  SELECT count(*) INTO n FROM stage s JOIN shop.treasury_ledger l ON l.id = s.id
   WHERE (l.type, l.usdc_delta, l.mana_delta, l.reference, l.metadata, l.created_at)
         IS DISTINCT FROM (s.type, s.usdc_delta, s.mana_delta, s.reference, s.metadata, s.created_at_ms);
  IF n > 0 THEN RAISE EXCEPTION 'shop-server/treasury_ledger: % entry ids are already held with other content', n;
  ELSE NULL; END IF;

  SELECT count(*) INTO n FROM stage s JOIN shop.treasury_ledger l
       ON l.type = s.type AND l.reference = s.reference AND l.id <> s.id
   WHERE s.reference IS NOT NULL;
  IF n > 0 THEN RAISE EXCEPTION 'shop-server/treasury_ledger: % (type, reference) pairs are already held under another id', n;
  ELSE NULL; END IF; END $check$;

INSERT INTO shop.treasury_ledger (id, type, usdc_delta, mana_delta, reference, metadata, created_at)
SELECT s.id, s.type, s.usdc_delta, s.mana_delta, s.reference, s.metadata, s.created_at_ms
FROM stage s
ON CONFLICT (id) DO NOTHING;
