-- Explicit source-local installation; runtime capture only reads this schema.
-- All functions are INVOKER. Grant every source writer required local rights
-- before resuming mutations. Nothing here contacts the target or Gatekeeper.
CREATE SCHEMA migration_capture_social_settings;
REVOKE ALL ON SCHEMA migration_capture_social_settings FROM PUBLIC;
CREATE TABLE migration_capture_social_settings.control (
    singleton boolean PRIMARY KEY DEFAULT true CHECK(singleton),
    version integer NOT NULL DEFAULT 1 CHECK(version=1),
    generation uuid NOT NULL DEFAULT gen_random_uuid() CHECK(generation<>'00000000-0000-0000-0000-000000000000'),
    position bigint NOT NULL DEFAULT 0 CHECK(position>=0)
);
INSERT INTO migration_capture_social_settings.control DEFAULT VALUES;
CREATE TABLE migration_capture_social_settings.scope (
    address text PRIMARY KEY CHECK(address ~ '^0x[0-9a-f]{40}$')
);
CREATE TABLE migration_capture_social_settings.transactions (
    position bigint PRIMARY KEY CHECK(position>0), xid xid8 NOT NULL UNIQUE,
    command jsonb CHECK(command IS NULL), created_at timestamptz NOT NULL DEFAULT clock_timestamp()
);
CREATE TABLE migration_capture_social_settings.changes (
    position bigint NOT NULL REFERENCES migration_capture_social_settings.transactions(position),
    ordinal bigint GENERATED ALWAYS AS IDENTITY,
    payload jsonb NOT NULL, PRIMARY KEY(position,ordinal)
);
CREATE FUNCTION migration_capture_social_settings.ensure_transaction() RETURNS bigint
LANGUAGE plpgsql SET search_path=pg_catalog,public AS $$
DECLARE p bigint;
BEGIN
    PERFORM pg_advisory_xact_lock(179321,3);
    SELECT position INTO p FROM migration_capture_social_settings.transactions WHERE xid=pg_current_xact_id();
    IF FOUND THEN RETURN p; END IF;
    UPDATE migration_capture_social_settings.control SET position=position+1 WHERE singleton RETURNING position INTO p;
    INSERT INTO migration_capture_social_settings.transactions(position,xid) VALUES(p,pg_current_xact_id());
    RETURN p;
END $$;
CREATE FUNCTION migration_capture_social_settings.before_statement() RETURNS trigger
LANGUAGE plpgsql SET search_path=pg_catalog,public AS $$
BEGIN
    IF TG_OP='TRUNCATE' THEN RAISE EXCEPTION 'TRUNCATE unsupported while settings capture is installed'; END IF;
    PERFORM pg_advisory_xact_lock(179321,3);
    RETURN NULL;
END $$;
CREATE FUNCTION migration_capture_social_settings.capture_row() RETURNS trigger
LANGUAGE plpgsql SET search_path=pg_catalog,public AS $$
DECLARE old_scoped boolean := false; new_scoped boolean := false; p bigint;
BEGIN
    IF TG_OP<>'INSERT' THEN
        SELECT EXISTS(SELECT 1 FROM migration_capture_social_settings.scope WHERE address=lower(OLD.address)) INTO old_scoped;
    END IF;
    IF TG_OP<>'DELETE' THEN
        SELECT EXISTS(SELECT 1 FROM migration_capture_social_settings.scope WHERE address=lower(NEW.address)) INTO new_scoped;
    END IF;
    IF NOT old_scoped AND NOT new_scoped THEN RETURN NULL; END IF;
    p:=migration_capture_social_settings.ensure_transaction();
    IF old_scoped AND (TG_OP='DELETE' OR NOT new_scoped OR OLD.address IS DISTINCT FROM NEW.address) THEN
        INSERT INTO migration_capture_social_settings.changes(position,payload) VALUES(p,jsonb_build_object('op','social_settings_delete','address',OLD.address));
    END IF;
    IF new_scoped THEN
        INSERT INTO migration_capture_social_settings.changes(position,payload) VALUES(p,jsonb_build_object(
            'op','social_settings_upsert','address',NEW.address,'private_messages_privacy',NEW.private_messages_privacy,
            'blocked_users_messages_visibility',NEW.blocked_users_messages_visibility,'show_situation_reactions',NEW.show_situation_reactions));
    END IF;
    RETURN NULL;
END $$;
CREATE TRIGGER migration_capture_settings_statement BEFORE INSERT OR UPDATE OR DELETE OR TRUNCATE ON public.social_settings
FOR EACH STATEMENT EXECUTE FUNCTION migration_capture_social_settings.before_statement();
CREATE TRIGGER migration_capture_settings_row AFTER INSERT OR UPDATE OR DELETE ON public.social_settings
FOR EACH ROW EXECUTE FUNCTION migration_capture_social_settings.capture_row();
