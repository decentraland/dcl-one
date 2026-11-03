-- Writes nothing: stops the run while upstream has unresolved refills (see
-- the extract). Stop upstream's refill job, let it finish them, run again.
DO $guard$ DECLARE n bigint; BEGIN
  SELECT count(*) INTO n FROM stage;
  IF n > 0 THEN RAISE EXCEPTION
    'shop-server/treasury_guard: % upstream refill intents are unresolved; let them settle or abort before importing the treasury', n;
  ELSE NULL; END IF; END $guard$;
