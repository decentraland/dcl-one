-- Operator-installed only. All locks are source-local; source commits never
-- wait for the follower. Statement locks precede row locks, establishing one
-- commit order across worlds ownership and quota changes, including direct SQL.
CREATE SCHEMA migration_worlds_capture;
REVOKE ALL ON SCHEMA migration_worlds_capture FROM PUBLIC;
CREATE TABLE migration_worlds_capture.control (
 singleton boolean PRIMARY KEY DEFAULT true CHECK(singleton),
 version integer NOT NULL DEFAULT 1 CHECK(version=1),
 generation uuid NOT NULL DEFAULT gen_random_uuid(),
 position bigint NOT NULL DEFAULT 0 CHECK(position>=0)
);
INSERT INTO migration_worlds_capture.control DEFAULT VALUES;
CREATE TABLE migration_worlds_capture.world_scope(world_name text PRIMARY KEY);
CREATE TABLE migration_worlds_capture.wallet_scope(wallet text PRIMARY KEY);
CREATE TABLE migration_worlds_capture.transactions(
 position bigint PRIMARY KEY,
 transaction_id bigint NOT NULL UNIQUE
);
CREATE TABLE migration_worlds_capture.changes(
 ordinal bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
 position bigint NOT NULL REFERENCES migration_worlds_capture.transactions(position),
 payload jsonb NOT NULL
);
CREATE INDEX worlds_capture_position_idx ON migration_worlds_capture.changes(position,ordinal);
CREATE FUNCTION migration_worlds_capture.position() RETURNS bigint LANGUAGE plpgsql
SECURITY DEFINER SET search_path=pg_catalog,public AS $$
DECLARE p bigint;
BEGIN
 PERFORM pg_advisory_xact_lock(179321,2);
 SELECT position INTO p FROM migration_worlds_capture.transactions WHERE transaction_id=txid_current();
 IF p IS NULL THEN
  UPDATE migration_worlds_capture.control SET position=position+1 RETURNING position INTO p;
  INSERT INTO migration_worlds_capture.transactions VALUES(p,txid_current());
 END IF;
 RETURN p;
END $$;
CREATE FUNCTION migration_worlds_capture.before_write() RETURNS trigger LANGUAGE plpgsql
SECURITY DEFINER SET search_path=pg_catalog,public AS $$
BEGIN
 IF TG_OP='TRUNCATE' THEN RAISE EXCEPTION 'TRUNCATE unsupported during Worlds capture'; END IF;
 PERFORM pg_advisory_xact_lock(179321,2);
 RETURN NULL;
END $$;
CREATE FUNCTION migration_worlds_capture.owner_change() RETURNS trigger LANGUAGE plpgsql
SECURITY DEFINER SET search_path=pg_catalog,public AS $$
DECLARE old_scoped boolean:=false; new_scoped boolean:=false; payload jsonb;
BEGIN
 IF TG_OP<>'INSERT' THEN
  SELECT EXISTS(SELECT 1 FROM migration_worlds_capture.world_scope WHERE world_name=lower(OLD.name)) INTO old_scoped;
 END IF;
 IF TG_OP<>'DELETE' THEN
  SELECT EXISTS(SELECT 1 FROM migration_worlds_capture.world_scope WHERE world_name=lower(NEW.name)) INTO new_scoped;
 END IF;
 IF NOT old_scoped AND NOT new_scoped THEN RETURN NULL; END IF;
 IF TG_OP<>'UPDATE' OR OLD.name IS DISTINCT FROM NEW.name THEN
  payload:=jsonb_build_object('op','unsupported','reason','world_identity_changed');
 ELSIF OLD.owner IS DISTINCT FROM NEW.owner THEN
  payload:=jsonb_build_object('op','owner','world',NEW.name,'before',OLD.owner,'after',NEW.owner);
 ELSE RETURN NULL;
 END IF;
 INSERT INTO migration_worlds_capture.changes(position,payload) VALUES(migration_worlds_capture.position(),payload);
 RETURN NULL;
END $$;
CREATE FUNCTION migration_worlds_capture.quota_change() RETURNS trigger LANGUAGE plpgsql
SECURITY DEFINER SET search_path=pg_catalog,public AS $$
DECLARE old_scoped boolean:=false; new_scoped boolean:=false; payload jsonb;
BEGIN
 IF TG_OP<>'INSERT' THEN
  SELECT EXISTS(SELECT 1 FROM migration_worlds_capture.wallet_scope WHERE wallet=lower(OLD.wallet)) INTO old_scoped;
 END IF;
 IF TG_OP<>'DELETE' THEN
  SELECT EXISTS(SELECT 1 FROM migration_worlds_capture.wallet_scope WHERE wallet=lower(NEW.wallet)) INTO new_scoped;
 END IF;
 IF NOT old_scoped AND NOT new_scoped THEN RETURN NULL; END IF;
 IF TG_OP='UPDATE' AND OLD.wallet IS DISTINCT FROM NEW.wallet THEN
  payload:=jsonb_build_object('op','unsupported','reason','quota_identity_changed');
 ELSE
  payload:=jsonb_build_object('op','quota','wallet',CASE WHEN TG_OP='DELETE' THEN OLD.wallet ELSE NEW.wallet END,
   'before',CASE WHEN TG_OP='INSERT' THEN NULL ELSE jsonb_build_object('created_at',OLD.created_at,'updated_at',OLD.updated_at) END,
   'after',CASE WHEN TG_OP='DELETE' THEN NULL ELSE jsonb_build_object('created_at',NEW.created_at,'updated_at',NEW.updated_at) END);
 END IF;
 INSERT INTO migration_worlds_capture.changes(position,payload) VALUES(migration_worlds_capture.position(),payload);
 RETURN NULL;
END $$;
CREATE TRIGGER migration_worlds_before BEFORE INSERT OR UPDATE OR DELETE OR TRUNCATE ON public.worlds
 FOR EACH STATEMENT EXECUTE FUNCTION migration_worlds_capture.before_write();
CREATE TRIGGER migration_worlds_owner AFTER INSERT OR UPDATE OR DELETE ON public.worlds
 FOR EACH ROW EXECUTE FUNCTION migration_worlds_capture.owner_change();
CREATE TRIGGER migration_worlds_quota_before BEFORE INSERT OR UPDATE OR DELETE OR TRUNCATE ON public.blocked
 FOR EACH STATEMENT EXECUTE FUNCTION migration_worlds_capture.before_write();
CREATE TRIGGER migration_worlds_quota AFTER INSERT OR UPDATE OR DELETE ON public.blocked
 FOR EACH ROW EXECUTE FUNCTION migration_worlds_capture.quota_change();
REVOKE ALL ON ALL TABLES IN SCHEMA migration_worlds_capture FROM PUBLIC;
REVOKE ALL ON ALL SEQUENCES IN SCHEMA migration_worlds_capture FROM PUBLIC;
REVOKE ALL ON ALL FUNCTIONS IN SCHEMA migration_worlds_capture FROM PUBLIC;
