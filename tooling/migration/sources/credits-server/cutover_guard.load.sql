-- Writes nothing: stops the run while upstream has unsettled USD intents or
-- Stripe orders in flight (see the extract). Stop upstream writes, let its
-- reconciler settle, then run again.
DO $guard$ DECLARE n bigint; BEGIN
  SELECT count(*) INTO n FROM stage;
  IF n > 0 THEN RAISE EXCEPTION
    'credits-server/cutover_guard: % upstream intents/orders are still in flight; settle them before importing balances', n;
  ELSE NULL; END IF; END $guard$;
