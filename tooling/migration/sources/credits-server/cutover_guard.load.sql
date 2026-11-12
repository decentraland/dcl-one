-- Writes nothing: stops on unresolved liabilities or payment facts.
-- Reconcile payment facts upstream; historical EXPIRED intents also need
-- a reviewed release-proof mechanism, not just time to settle (see MAPPING).
DO $guard$ DECLARE n bigint; BEGIN
  SELECT count(*) INTO n FROM stage;
  IF n > 0 THEN RAISE EXCEPTION
    'credits-server/cutover_guard: % upstream liabilities or payment facts are unresolved; reconcile them before importing balances (EXPIRED intents require an independently verified release mechanism)', n;
  ELSE NULL; END IF; END $guard$;
