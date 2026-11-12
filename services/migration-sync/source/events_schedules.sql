-- Explicit installation in the Events source DB. No target calls or event jobs.
CREATE SCHEMA migration_capture_events;
REVOKE ALL ON SCHEMA migration_capture_events FROM PUBLIC;
CREATE TABLE migration_capture_events.control (
    singleton boolean PRIMARY KEY DEFAULT true CHECK(singleton),
    version integer NOT NULL DEFAULT 1 CHECK(version=1),
    generation uuid NOT NULL DEFAULT gen_random_uuid() CHECK(generation<>'00000000-0000-0000-0000-000000000000'),
    position bigint NOT NULL DEFAULT 0 CHECK(position>=0)
);
INSERT INTO migration_capture_events.control DEFAULT VALUES;
CREATE TABLE migration_capture_events.scope(entity_id uuid PRIMARY KEY);
CREATE TABLE migration_capture_events.transactions (
    position bigint PRIMARY KEY CHECK(position>0), xid xid8 NOT NULL UNIQUE,
    command jsonb CHECK(command IS NULL), created_at timestamptz NOT NULL DEFAULT clock_timestamp()
);
CREATE TABLE migration_capture_events.changes (
    position bigint NOT NULL REFERENCES migration_capture_events.transactions(position),
    ordinal bigint GENERATED ALWAYS AS IDENTITY, payload jsonb NOT NULL,
    PRIMARY KEY(position,ordinal)
);
CREATE FUNCTION migration_capture_events.ensure_transaction() RETURNS bigint
LANGUAGE plpgsql SET search_path=pg_catalog,public AS $$
DECLARE p bigint;
BEGIN
    PERFORM pg_advisory_xact_lock(179321,2);
    SELECT position INTO p FROM migration_capture_events.transactions WHERE xid=pg_current_xact_id();
    IF FOUND THEN RETURN p; END IF;
    UPDATE migration_capture_events.control SET position=position+1 WHERE singleton RETURNING position INTO p;
    INSERT INTO migration_capture_events.transactions(position,xid) VALUES(p,pg_current_xact_id());
    RETURN p;
END $$;
CREATE FUNCTION migration_capture_events.before_statement() RETURNS trigger
LANGUAGE plpgsql SET search_path=pg_catalog,public AS $$
BEGIN
    PERFORM pg_advisory_xact_lock(179321,2);
    RETURN NULL;
END $$;
CREATE FUNCTION migration_capture_events.no_truncate() RETURNS trigger
LANGUAGE plpgsql SET search_path=pg_catalog,public AS $$
BEGIN
    RAISE EXCEPTION 'TRUNCATE is unsupported while schedule capture is installed; use captured DELETE';
END $$;
CREATE FUNCTION migration_capture_events.capture_schedule() RETURNS trigger
LANGUAGE plpgsql SET search_path=pg_catalog,public AS $$
DECLARE p bigint; old_scoped boolean:=false; new_scoped boolean:=false;
BEGIN
    IF TG_OP<>'INSERT' THEN
        SELECT EXISTS(SELECT 1 FROM migration_capture_events.scope WHERE entity_id=OLD.id) INTO old_scoped;
    END IF;
    IF TG_OP<>'DELETE' THEN
        SELECT EXISTS(SELECT 1 FROM migration_capture_events.scope WHERE entity_id=NEW.id) INTO new_scoped;
    END IF;
    IF NOT old_scoped AND NOT new_scoped THEN RETURN NULL; END IF;
    p:=migration_capture_events.ensure_transaction();
    IF old_scoped AND (TG_OP='DELETE' OR NOT new_scoped OR OLD.id IS DISTINCT FROM NEW.id) THEN
        INSERT INTO migration_capture_events.changes(position,payload) VALUES(p,jsonb_build_object('op','schedule_delete','id',OLD.id));
    END IF;
    IF new_scoped THEN
        INSERT INTO migration_capture_events.changes(position,payload) VALUES(p,
            jsonb_build_object('op','schedule_upsert','schedule',jsonb_build_object(
                'id',NEW.id,'name',NEW.name,'description',NEW.description,'image',NEW.image,'theme',NEW.theme,
                'background',to_jsonb(NEW.background),'active',NEW.active,
                'active_since',to_char(NEW.active_since AT TIME ZONE 'UTC','YYYY-MM-DD"T"HH24:MI:SS.US"Z"'),
                'active_until',to_char(NEW.active_until AT TIME ZONE 'UTC','YYYY-MM-DD"T"HH24:MI:SS.US"Z"'),
                'created_at',to_char(NEW.created_at AT TIME ZONE 'UTC','YYYY-MM-DD"T"HH24:MI:SS.US"Z"'),
                'updated_at',to_char(NEW.updated_at AT TIME ZONE 'UTC','YYYY-MM-DD"T"HH24:MI:SS.US"Z"'))));
    END IF;
    RETURN NULL;
END $$;
CREATE TRIGGER migration_capture_statement BEFORE INSERT OR UPDATE OR DELETE ON public.schedule
FOR EACH STATEMENT EXECUTE FUNCTION migration_capture_events.before_statement();
CREATE TRIGGER migration_capture_row AFTER INSERT OR UPDATE OR DELETE ON public.schedule
FOR EACH ROW EXECUTE FUNCTION migration_capture_events.capture_schedule();
CREATE TRIGGER migration_capture_no_truncate BEFORE TRUNCATE ON public.schedule
FOR EACH STATEMENT EXECUTE FUNCTION migration_capture_events.no_truncate();
