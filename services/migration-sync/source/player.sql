-- Explicit rehearsal installation only. Install under a dedicated owner in one transaction,
-- while application writes are stopped. Never install automatically on service startup.
CREATE SCHEMA migration_player_capture;
REVOKE ALL ON SCHEMA migration_player_capture FROM PUBLIC;
CREATE TABLE migration_player_capture.control (
 singleton boolean PRIMARY KEY CHECK(singleton), version integer NOT NULL CHECK(version=1),
 generation uuid NOT NULL CHECK(generation<>'00000000-0000-0000-0000-000000000000'), position bigint NOT NULL DEFAULT 0 CHECK(position>=0),
 sealed boolean NOT NULL DEFAULT false
);
INSERT INTO migration_player_capture.control VALUES(true,1,gen_random_uuid(),0,false);
CREATE TABLE migration_player_capture.scope (
 world_name text NOT NULL CHECK(length(world_name) BETWEEN 5 AND 255 AND world_name=lower(world_name) AND world_name LIKE '%.eth'),
 place_id uuid NOT NULL CHECK(place_id<>'00000000-0000-0000-0000-000000000000'),
 scene_id text NOT NULL CHECK(length(scene_id) BETWEEN 1 AND 256), PRIMARY KEY(world_name,place_id)
);
CREATE TABLE migration_player_capture.transactions (
 position bigint PRIMARY KEY, source_txid bigint NOT NULL UNIQUE,
 command jsonb, unclassified boolean NOT NULL DEFAULT false
);
-- Only active while apply_command runs. Application roles cannot write this table.
CREATE TABLE migration_player_capture.active_command (
 source_txid bigint PRIMARY KEY, world_name text NOT NULL, place_id uuid NOT NULL,
 op text NOT NULL, player_address text, key text, value_text text
);
CREATE FUNCTION migration_player_capture.lock_capture() RETURNS void LANGUAGE sql
SECURITY DEFINER SET search_path=pg_catalog AS $$ SELECT pg_advisory_xact_lock(179323,1) $$;
CREATE FUNCTION migration_player_capture.ensure_transaction() RETURNS bigint LANGUAGE plpgsql
SECURITY DEFINER SET search_path=pg_catalog AS $$
DECLARE p bigint;
BEGIN
 PERFORM migration_player_capture.lock_capture();
 SELECT position INTO p FROM migration_player_capture.transactions WHERE source_txid=txid_current();
 IF p IS NULL THEN
  UPDATE migration_player_capture.control SET position=position+1 WHERE singleton RETURNING position INTO p;
  INSERT INTO migration_player_capture.transactions(position,source_txid) VALUES(p,txid_current());
 END IF;
 RETURN p;
END $$;
CREATE FUNCTION migration_player_capture.statement_guard() RETURNS trigger LANGUAGE plpgsql
SECURITY DEFINER SET search_path=pg_catalog AS $$
BEGIN
 IF TG_OP='TRUNCATE' THEN RAISE EXCEPTION 'TRUNCATE unsupported while player capture is installed'; END IF;
 PERFORM migration_player_capture.lock_capture();
 RETURN NULL;
END $$;
CREATE FUNCTION migration_player_capture.row_evidence() RETURNS trigger LANGUAGE plpgsql
SECURITY DEFINER SET search_path=pg_catalog AS $$
DECLARE old_scoped boolean:=false; new_scoped boolean:=false; trusted boolean:=false; p bigint;
BEGIN
 IF TG_OP<>'INSERT' THEN
  SELECT EXISTS(SELECT 1 FROM migration_player_capture.scope s WHERE s.world_name=OLD.world_name AND s.place_id=OLD.place_id) INTO old_scoped;
 END IF;
 IF TG_OP<>'DELETE' THEN
  SELECT EXISTS(SELECT 1 FROM migration_player_capture.scope s WHERE s.world_name=NEW.world_name AND s.place_id=NEW.place_id) INTO new_scoped;
 END IF;
 IF NOT old_scoped AND NOT new_scoped THEN RETURN NULL; END IF;
 SELECT EXISTS(SELECT 1 FROM migration_player_capture.active_command a WHERE a.source_txid=txid_current()
  AND (TG_OP='INSERT' OR (a.world_name=OLD.world_name AND a.place_id=OLD.place_id))
  AND (TG_OP='DELETE' OR (a.world_name=NEW.world_name AND a.place_id=NEW.place_id))
  AND (
   (a.op='set' AND TG_OP IN ('INSERT','UPDATE')
    AND NEW.player_address=a.player_address AND NEW.key=a.key AND NEW.value=a.value_text::jsonb
    AND NEW.value_size=octet_length(a.value_text)
    AND (TG_OP='INSERT' OR (OLD.player_address=NEW.player_address AND OLD.key=NEW.key)))
   OR (a.op='delete' AND TG_OP='DELETE' AND OLD.player_address=a.player_address AND OLD.key=a.key)
   OR (a.op='clear_player' AND TG_OP='DELETE' AND OLD.player_address=a.player_address)
   OR (a.op='clear_scene' AND TG_OP='DELETE')
  )) INTO trusted;
 p:=migration_player_capture.ensure_transaction();
 IF NOT trusted THEN UPDATE migration_player_capture.transactions SET unclassified=true WHERE position=p; END IF;
 RETURN NULL;
END $$;
CREATE TRIGGER migration_player_statement BEFORE INSERT OR UPDATE OR DELETE OR TRUNCATE ON public.player_storage
FOR EACH STATEMENT EXECUTE FUNCTION migration_player_capture.statement_guard();
CREATE TRIGGER migration_player_row AFTER INSERT OR UPDATE OR DELETE ON public.player_storage
FOR EACH ROW EXECUTE FUNCTION migration_player_capture.row_evidence();

-- The ordinary application already authorized and quota-validated this operation.
-- The function preserves that operation and records its accepted identity atomically.
CREATE FUNCTION migration_player_capture.apply_command(
 op text, world_name text, place_id uuid, scene_id text, player_address text, key text, value_text text
) RETURNS void LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
DECLARE binding text; p bigint; created timestamp; updated timestamp; sz integer; accepted jsonb;
 accepted_at timestamp := date_trunc('milliseconds', clock_timestamp() AT TIME ZONE 'UTC');
BEGIN
 IF op IS NULL OR op NOT IN ('set','delete','clear_player','clear_scene') THEN RAISE EXCEPTION 'unsupported player command'; END IF;
 IF world_name IS NULL OR place_id IS NULL THEN RAISE EXCEPTION 'missing player command scope'; END IF;
 IF (op='clear_scene' AND player_address IS NOT NULL)
  OR (op<>'clear_scene' AND (player_address IS NULL OR player_address !~ '^0x[0-9a-f]{40}$'))
  OR (op IN ('set','delete') AND (key IS NULL OR length(key) NOT BETWEEN 1 AND 255))
  OR (op IN ('clear_player','clear_scene') AND key IS NOT NULL)
  OR (op='set' AND value_text IS NULL) OR (op<>'set' AND value_text IS NOT NULL) THEN
  RAISE EXCEPTION 'invalid player command fields';
 END IF;
 PERFORM migration_player_capture.lock_capture();
 IF NOT (SELECT sealed FROM migration_player_capture.control WHERE singleton) THEN RAISE EXCEPTION 'player capture scope is not sealed'; END IF;
 SELECT s.scene_id INTO binding FROM migration_player_capture.scope s
  WHERE s.world_name=apply_command.world_name AND s.place_id=apply_command.place_id;
 IF binding IS NOT NULL THEN
  p:=migration_player_capture.ensure_transaction();
  -- Missing scene evidence never changes production authorization. It blocks the follower.
  IF binding IS DISTINCT FROM scene_id OR EXISTS(SELECT 1 FROM migration_player_capture.transactions t WHERE t.position=p AND t.command IS NOT NULL) THEN
   UPDATE migration_player_capture.transactions SET unclassified=true WHERE position=p;
  END IF;
  INSERT INTO migration_player_capture.active_command VALUES(txid_current(),world_name,place_id,op,player_address,key,value_text);
 END IF;
 IF op='set' THEN
  sz:=octet_length(value_text);
  INSERT INTO public.player_storage(world_name,place_id,player_address,key,value,value_size,created_at,updated_at)
  VALUES(world_name,place_id,player_address,key,value_text::jsonb,sz,accepted_at,accepted_at)
  ON CONFLICT ON CONSTRAINT player_storage_pkey DO UPDATE SET value=EXCLUDED.value,value_size=EXCLUDED.value_size,updated_at=EXCLUDED.updated_at
  RETURNING created_at,updated_at INTO created,updated;
 ELSIF op='delete' THEN
  DELETE FROM public.player_storage v WHERE v.world_name=apply_command.world_name AND v.place_id=apply_command.place_id
    AND v.player_address=apply_command.player_address AND v.key=apply_command.key;
 ELSIF op='clear_player' THEN
  DELETE FROM public.player_storage v WHERE v.world_name=apply_command.world_name AND v.place_id=apply_command.place_id AND v.player_address=apply_command.player_address;
 ELSE
  DELETE FROM public.player_storage v WHERE v.world_name=apply_command.world_name AND v.place_id=apply_command.place_id;
 END IF;
 IF binding IS NOT NULL THEN
  DELETE FROM migration_player_capture.active_command WHERE source_txid=txid_current();
  accepted:=jsonb_build_object('version',1,'op',op,'world_name',world_name,'place_id',place_id::text,'scene_id',scene_id,
   'player_address',player_address,'key',key,'value_text',value_text,'value_size',sz,
   'created_at',to_char(created,'YYYY-MM-DD"T"HH24:MI:SS.US"Z"'),'updated_at',to_char(updated,'YYYY-MM-DD"T"HH24:MI:SS.US"Z"'));
  UPDATE migration_player_capture.transactions SET command=accepted WHERE position=p AND NOT unclassified;
 END IF;
END $$;
-- Scope is populated once by the installer, then sealed before application opt-in.
CREATE FUNCTION migration_player_capture.scope_guard() RETURNS trigger LANGUAGE plpgsql
SECURITY DEFINER SET search_path=pg_catalog AS $$
BEGIN
 PERFORM migration_player_capture.lock_capture();
 IF (SELECT sealed FROM migration_player_capture.control WHERE singleton) THEN
  RAISE EXCEPTION 'player capture scope is immutable';
 END IF;
 RETURN NULL;
END $$;
CREATE TRIGGER migration_player_scope_guard BEFORE INSERT OR UPDATE OR DELETE OR TRUNCATE ON migration_player_capture.scope
FOR EACH STATEMENT EXECUTE FUNCTION migration_player_capture.scope_guard();
CREATE FUNCTION migration_player_capture.seal_scope() RETURNS void LANGUAGE plpgsql
SECURITY DEFINER SET search_path=pg_catalog AS $$
BEGIN
 PERFORM migration_player_capture.lock_capture();
 IF NOT EXISTS(SELECT 1 FROM migration_player_capture.scope) THEN RAISE EXCEPTION 'empty player capture scope'; END IF;
 IF (SELECT count(*) FROM migration_player_capture.scope)>1000 THEN RAISE EXCEPTION 'player capture scope too large'; END IF;
 UPDATE migration_player_capture.control SET sealed=true WHERE singleton;
END $$;
REVOKE ALL ON ALL TABLES IN SCHEMA migration_player_capture FROM PUBLIC;
REVOKE ALL ON ALL FUNCTIONS IN SCHEMA migration_player_capture FROM PUBLIC;
