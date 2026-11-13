-- Explicit opt-in installation on the source Places database. The application
-- owner installs this once and grants only the intended application/capture roles.
-- No function contacts the destination. The per-transaction lock establishes
-- commit order before any favorite row mutation can acquire its row locks.
CREATE SCHEMA migration_capture;
REVOKE ALL ON SCHEMA migration_capture FROM PUBLIC;

CREATE TABLE migration_capture.control (
    singleton boolean PRIMARY KEY DEFAULT true CHECK (singleton),
    version integer NOT NULL DEFAULT 4 CHECK (version = 4),
    generation uuid NOT NULL DEFAULT gen_random_uuid() CHECK (generation <> '00000000-0000-0000-0000-000000000000'),
    position bigint NOT NULL DEFAULT 0 CHECK (position >= 0)
);
INSERT INTO migration_capture.control DEFAULT VALUES;
CREATE TABLE migration_capture.scope (entity_id uuid PRIMARY KEY);
CREATE TABLE migration_capture.transactions (
    position bigint PRIMARY KEY CHECK (position > 0),
    xid xid8 NOT NULL UNIQUE,
    command jsonb,
    created_at timestamptz NOT NULL DEFAULT clock_timestamp()
);
CREATE TABLE migration_capture.changes (
    position bigint NOT NULL REFERENCES migration_capture.transactions(position),
    ordinal bigint GENERATED ALWAYS AS IDENTITY,
    payload jsonb NOT NULL,
    PRIMARY KEY (position, ordinal)
);
CREATE TABLE migration_capture.command_receipts (
    operation_id uuid PRIMARY KEY,
    request_hash text NOT NULL CHECK (request_hash ~ '^[0-9a-f]{64}$'),
    actor text NOT NULL CHECK (actor ~ '^0x[0-9a-f]{40}$'),
    entity_id uuid NOT NULL REFERENCES migration_capture.scope(entity_id),
    kind text NOT NULL DEFAULT 'favorite' CHECK(kind IN ('favorite','like')),
    favorite boolean,
    vote boolean,
    position bigint NOT NULL REFERENCES migration_capture.transactions(position),
    response jsonb NOT NULL,
    created_at timestamptz NOT NULL DEFAULT clock_timestamp()
);

CREATE FUNCTION migration_capture.ensure_transaction() RETURNS bigint
LANGUAGE plpgsql SET search_path = pg_catalog, public AS $$
DECLARE p bigint;
BEGIN
    PERFORM pg_advisory_xact_lock(179321, 1);
    SELECT position INTO p FROM migration_capture.transactions WHERE xid = pg_current_xact_id();
    IF FOUND THEN RETURN p; END IF;
    UPDATE migration_capture.control SET position=position+1 WHERE singleton RETURNING position INTO p;
    INSERT INTO migration_capture.transactions(position,xid) VALUES(p,pg_current_xact_id());
    RETURN p;
END $$;

CREATE FUNCTION migration_capture.record_command(value jsonb) RETURNS bigint
LANGUAGE plpgsql SET search_path = pg_catalog, public AS $$
DECLARE p bigint; previous jsonb;
BEGIN
    p := migration_capture.ensure_transaction();
    SELECT command INTO previous FROM migration_capture.transactions WHERE position=p;
    IF previous IS NOT NULL AND previous IS DISTINCT FROM value THEN
        RAISE EXCEPTION 'one capture transaction cannot contain different commands';
    END IF;
    UPDATE migration_capture.transactions SET command=value WHERE position=p;
    RETURN p;
END $$;

CREATE FUNCTION migration_capture.favorite_statement() RETURNS trigger
LANGUAGE plpgsql SET search_path = pg_catalog, public AS $$
BEGIN
    PERFORM pg_advisory_xact_lock(179321, 1);
    RETURN NULL;
END $$;
CREATE FUNCTION migration_capture.favorite_truncate() RETURNS trigger
LANGUAGE plpgsql SET search_path = pg_catalog, public AS $$
BEGIN
    RAISE EXCEPTION 'TRUNCATE is unsupported while favorites capture is installed; use captured DELETE';
END $$;
CREATE FUNCTION migration_capture.favorite_row() RETURNS trigger
LANGUAGE plpgsql SET search_path = pg_catalog, public AS $$
DECLARE p bigint; old_scoped boolean := false; new_scoped boolean := false;
BEGIN
    IF TG_OP <> 'INSERT' THEN
        SELECT EXISTS(SELECT 1 FROM migration_capture.scope WHERE entity_id::text=lower(OLD.entity_id::text)) INTO old_scoped;
    END IF;
    IF TG_OP <> 'DELETE' THEN
        SELECT EXISTS(SELECT 1 FROM migration_capture.scope WHERE entity_id::text=lower(NEW.entity_id::text)) INTO new_scoped;
    END IF;
    IF NOT old_scoped AND NOT new_scoped THEN RETURN NULL; END IF;
    p := migration_capture.ensure_transaction();
    IF old_scoped AND (TG_OP='DELETE' OR NOT new_scoped OR (OLD."user",OLD.entity_id) IS DISTINCT FROM (NEW."user",NEW.entity_id)) THEN
        INSERT INTO migration_capture.changes(position,payload) VALUES(p,
            jsonb_build_object('op','delete','user',OLD."user"::text,'entity_id',OLD.entity_id::text));
    END IF;
    IF new_scoped THEN
        INSERT INTO migration_capture.changes(position,payload) VALUES(p,
            jsonb_build_object('op','upsert','user',NEW."user"::text,'entity_id',NEW.entity_id::text,
                'user_activity',NEW.user_activity,'created_at',
                to_char(NEW.created_at AT TIME ZONE 'UTC','YYYY-MM-DD"T"HH24:MI:SS.US"Z"')));
    END IF;
    RETURN NULL;
END $$;
CREATE TRIGGER migration_capture_statement BEFORE INSERT OR UPDATE OR DELETE ON public.user_favorites
FOR EACH STATEMENT EXECUTE FUNCTION migration_capture.favorite_statement();
CREATE TRIGGER migration_capture_row AFTER INSERT OR UPDATE OR DELETE ON public.user_favorites
FOR EACH ROW EXECUTE FUNCTION migration_capture.favorite_row();
CREATE TRIGGER migration_capture_no_truncate BEFORE TRUNCATE ON public.user_favorites
FOR EACH STATEMENT EXECUTE FUNCTION migration_capture.favorite_truncate();

CREATE FUNCTION migration_capture.like_row() RETURNS trigger
LANGUAGE plpgsql SET search_path = pg_catalog, public AS $$
DECLARE p bigint; old_scoped boolean := false; new_scoped boolean := false;
BEGIN
    IF TG_OP <> 'INSERT' THEN
        SELECT EXISTS(SELECT 1 FROM migration_capture.scope WHERE entity_id::text=lower(OLD.entity_id::text)) INTO old_scoped;
    END IF;
    IF TG_OP <> 'DELETE' THEN
        SELECT EXISTS(SELECT 1 FROM migration_capture.scope WHERE entity_id::text=lower(NEW.entity_id::text)) INTO new_scoped;
    END IF;
    IF NOT old_scoped AND NOT new_scoped THEN RETURN NULL; END IF;
    p := migration_capture.ensure_transaction();
    IF old_scoped AND (TG_OP='DELETE' OR NOT new_scoped OR (OLD."user",OLD.entity_id) IS DISTINCT FROM (NEW."user",NEW.entity_id)) THEN
        INSERT INTO migration_capture.changes(position,payload) VALUES(p,
            jsonb_build_object('op','like_delete','user',OLD."user"::text,'entity_id',OLD.entity_id::text));
    END IF;
    IF new_scoped THEN
        INSERT INTO migration_capture.changes(position,payload) VALUES(p,
            jsonb_build_object('op','like_upsert','user',NEW."user"::text,'entity_id',NEW.entity_id::text,
                'like',NEW."like",'user_activity',NEW.user_activity,'updated_at',
                to_char(NEW.updated_at AT TIME ZONE 'UTC','YYYY-MM-DD"T"HH24:MI:SS.US"Z"'),'created_at',
                to_char(NEW.created_at AT TIME ZONE 'UTC','YYYY-MM-DD"T"HH24:MI:SS.US"Z"')));
    END IF;
    RETURN NULL;
END $$;
CREATE TRIGGER migration_capture_statement BEFORE INSERT OR UPDATE OR DELETE ON public.user_likes
FOR EACH STATEMENT EXECUTE FUNCTION migration_capture.favorite_statement();
CREATE TRIGGER migration_capture_row AFTER INSERT OR UPDATE OR DELETE ON public.user_likes
FOR EACH ROW EXECUTE FUNCTION migration_capture.like_row();
CREATE TRIGGER migration_capture_no_truncate BEFORE TRUNCATE ON public.user_likes
FOR EACH STATEMENT EXECUTE FUNCTION migration_capture.favorite_truncate();

-- Called only after the source HTTP handler has authenticated the actor and
-- validated the request. One statement owns mutation, aggregate and receipt.
CREATE FUNCTION migration_capture.execute_favorite(
    operation_id uuid, request_hash text, actor text, entity_id uuid,
    favorite boolean, user_activity double precision) RETURNS jsonb
LANGUAGE plpgsql SET search_path = pg_catalog, public AS $$
DECLARE existing migration_capture.command_receipts%ROWTYPE;
        p bigint; expected jsonb; answer jsonb; total bigint;
BEGIN
    IF operation_id IS NULL OR operation_id='00000000-0000-0000-0000-000000000000'::uuid
       OR request_hash IS NULL OR request_hash !~ '^[0-9a-f]{64}$'
       OR actor IS NULL OR actor !~ '^0x[0-9a-f]{40}$'
       OR favorite IS NULL OR user_activity IS NULL OR user_activity IN ('NaN'::float8,'Infinity'::float8,'-Infinity'::float8)
       OR user_activity <> trunc(user_activity) OR user_activity NOT BETWEEN -2147483648 AND 2147483647 THEN
        RAISE EXCEPTION 'invalid favorite command';
    END IF;
    PERFORM pg_advisory_xact_lock(179321, 1);
    SELECT * INTO existing FROM migration_capture.command_receipts r WHERE r.operation_id=execute_favorite.operation_id;
    IF FOUND THEN
        IF existing.kind <> 'favorite' OR existing.request_hash <> request_hash OR existing.actor <> actor
           OR existing.entity_id <> entity_id OR existing.favorite <> favorite THEN
            RAISE EXCEPTION 'operation identity conflicts with retained receipt';
        END IF;
        RETURN existing.response;
    END IF;
    IF NOT EXISTS(SELECT 1 FROM migration_capture.scope s WHERE s.entity_id=execute_favorite.entity_id)
       OR NOT EXISTS(SELECT 1 FROM public.places p WHERE p.id::text=execute_favorite.entity_id::text AND NOT p.world AND NOT p.disabled) THEN
        RAISE EXCEPTION 'favorite entity is outside the active capture scope';
    END IF;
    p := migration_capture.ensure_transaction();
    IF favorite THEN
        INSERT INTO public.user_favorites("user",entity_id,user_activity,created_at)
        VALUES(actor,entity_id::text,user_activity::integer,clock_timestamp())
        ON CONFLICT DO NOTHING;
    ELSE
        DELETE FROM public.user_favorites f WHERE f."user"=actor AND f.entity_id::text=execute_favorite.entity_id::text;
    END IF;
    SELECT count(*) INTO total FROM public.user_favorites f WHERE f.entity_id::text=execute_favorite.entity_id::text;
    UPDATE public.places SET favorites=total WHERE id::text=execute_favorite.entity_id::text;
    SELECT jsonb_build_object('user_activity',f.user_activity,'created_at',
        to_char(f.created_at AT TIME ZONE 'UTC','YYYY-MM-DD"T"HH24:MI:SS.US"Z"')) INTO expected
    FROM public.user_favorites f WHERE f."user"=actor AND f.entity_id::text=execute_favorite.entity_id::text;
    answer := jsonb_build_object('favorites',total,'user_favorite',favorite);
    PERFORM migration_capture.record_command(jsonb_build_object(
        'operation_id',operation_id,'request_hash',request_hash,'user',actor,
        'entity_id',entity_id,'favorite',favorite,'user_activity',user_activity,'expected',expected));
    INSERT INTO migration_capture.command_receipts(operation_id,request_hash,actor,entity_id,favorite,position,response)
    VALUES(operation_id,request_hash,actor,entity_id,favorite,p,answer);
    RETURN answer;
END $$;

-- Accepted vote setter. The captured command, source facts/counters and retry
-- response commit together. Same-vote requests preserve the existing fact.
CREATE FUNCTION migration_capture.execute_like(
    operation_id uuid, request_hash text, actor text, entity_id uuid,
    vote boolean, user_activity double precision) RETURNS jsonb
LANGUAGE plpgsql SET search_path=pg_catalog,public AS $$
#variable_conflict use_column
DECLARE existing migration_capture.command_receipts%ROWTYPE; p bigint; expected jsonb; answer jsonb;
        total_likes bigint; total_dislikes bigint; active_likes bigint; active_dislikes bigint; n bigint;
BEGIN
    IF operation_id IS NULL OR operation_id='00000000-0000-0000-0000-000000000000'::uuid
       OR request_hash IS NULL OR request_hash !~ '^[0-9a-f]{64}$'
       OR actor IS NULL OR actor !~ '^0x[0-9a-f]{40}$'
       OR user_activity IS NULL OR user_activity IN ('NaN'::float8,'Infinity'::float8,'-Infinity'::float8)
       OR user_activity<>trunc(user_activity) OR user_activity NOT BETWEEN -2147483648 AND 2147483647 THEN
        RAISE EXCEPTION 'invalid vote command';
    END IF;
    PERFORM pg_advisory_xact_lock(179321,1);
    SELECT * INTO existing FROM migration_capture.command_receipts r WHERE r.operation_id=execute_like.operation_id;
    IF FOUND THEN
        IF existing.kind<>'like' OR existing.request_hash<>request_hash OR existing.actor<>actor
           OR existing.entity_id<>entity_id OR existing.vote IS DISTINCT FROM vote THEN
            RAISE EXCEPTION 'operation identity conflicts with retained receipt';
        END IF;
        RETURN existing.response;
    END IF;
    IF NOT EXISTS(SELECT 1 FROM migration_capture.scope s WHERE s.entity_id=execute_like.entity_id)
       OR NOT EXISTS(SELECT 1 FROM public.places p WHERE p.id::text=execute_like.entity_id::text AND NOT p.world AND NOT p.disabled) THEN
        RAISE EXCEPTION 'vote entity is outside the active capture scope';
    END IF;
    p:=migration_capture.ensure_transaction();
    IF vote IS NULL THEN
        DELETE FROM public.user_likes l WHERE l."user"=actor AND l.entity_id::text=execute_like.entity_id::text;
    ELSE
        INSERT INTO public.user_likes("user",entity_id,"like",user_activity,created_at,updated_at)
        VALUES(actor,entity_id::text,vote,user_activity::integer,statement_timestamp(),statement_timestamp())
        ON CONFLICT("user",entity_id) DO UPDATE SET "like"=EXCLUDED."like",user_activity=EXCLUDED.user_activity,updated_at=EXCLUDED.updated_at
        WHERE public.user_likes."like" IS DISTINCT FROM EXCLUDED."like";
    END IF;
    SELECT count(*) FILTER(WHERE l."like"),count(*) FILTER(WHERE NOT l."like"),
        count(*) FILTER(WHERE l."like" AND l.user_activity>=100),count(*) FILTER(WHERE NOT l."like" AND l.user_activity>=100)
    INTO total_likes,total_dislikes,active_likes,active_dislikes
    FROM public.user_likes l WHERE l.entity_id::text=execute_like.entity_id::text;
    n:=active_likes+active_dislikes;
    UPDATE public.places SET likes=total_likes,dislikes=total_dislikes,
      like_rate=CASE WHEN n=0 THEN NULL ELSE active_likes::float8/n END,
      like_score=CASE WHEN n=0 THEN NULL ELSE ((active_likes+1.9208)/n-1.96*sqrt((active_likes*active_dislikes)/n+0.9604)/n)/(1+3.8416/n) END
    WHERE id::text=execute_like.entity_id::text;
    SELECT jsonb_build_object('like',l."like",'user_activity',l.user_activity,
      'created_at',to_char(l.created_at AT TIME ZONE 'UTC','YYYY-MM-DD"T"HH24:MI:SS.US"Z"'),
      'updated_at',to_char(l.updated_at AT TIME ZONE 'UTC','YYYY-MM-DD"T"HH24:MI:SS.US"Z"')) INTO expected
    FROM public.user_likes l WHERE l."user"=actor AND l.entity_id::text=execute_like.entity_id::text;
    answer:=jsonb_build_object('likes',total_likes,'dislikes',total_dislikes,'user_like',COALESCE(vote,false),'user_dislike',COALESCE(NOT vote,false));
    PERFORM migration_capture.record_command(jsonb_build_object('operation_id',operation_id,'request_hash',request_hash,'user',actor,
      'entity_id',entity_id,'like',vote,'user_activity',user_activity,'expected',expected));
    INSERT INTO migration_capture.command_receipts(operation_id,request_hash,actor,entity_id,kind,vote,position,response)
    VALUES(operation_id,request_hash,actor,entity_id,'like',vote,p,answer);
    RETURN answer;
END $$;
