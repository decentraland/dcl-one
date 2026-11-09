-- Typed read columns for the explore lists. The guarded reads of the raw
-- document that every places and events list computed per row and per sort
-- (query.rs raw_float8_sql, raw_int_sql, the timestamp guard, the textsearch
-- expressions; events/mod.rs EFF_NEXT_FINISH_SQL) are stored once at write
-- time, so an index orders a page and a projection reads a column. The
-- mirrors and the local writers are unchanged: a generated column is never
-- written. One ALTER TABLE per table adds its columns, so each table is
-- rewritten once (a statement per column would rewrite it once per column);
-- the indexes over the new columns and the wider explore.place_indexed view
-- come from schemas/10_explore.sql. The expression indexes of the first
-- release's like_score order are dropped before the place rewrite: no
-- statement reads them since like_score_num, and the rewrite would rebuild
-- them. The three functions the columns call are created here first, under
-- their hash names (src/explore/schema_functions.rs); a database that has
-- one skips its statement.
-- An ISO 8601 date-time string as a UTC timestamp, NULL for anything else.
-- The guard admits the forms query.rs RAW_TIMESTAMP_IS_CASTABLE_RE names (year
-- first, 24-hour time, an optional zone), which every DateStyle reads the same
-- way, so the function is immutable and a generated column may call it. A
-- zone-less string is read as UTC whatever the session's TimeZone: only a
-- value with a time field can carry a zone, so the zone test requires the
-- minutes or seconds digits before it and a date's -DD is never an offset.
CREATE FUNCTION explore.utc_timestamp_c37f040ae6c2(value text) RETURNS timestamp without time zone
    LANGUAGE sql IMMUTABLE PARALLEL SAFE STRICT
    RETURN CASE WHEN value ~ '^((000[1-9]|00[1-9][0-9]|0[1-9][0-9]{2}|[1-9][0-9]{3})-((0[13578]|1[02])-(0[1-9]|[12][0-9]|3[01])|(0[469]|11)-(0[1-9]|[12][0-9]|30)|02-(0[1-9]|1[0-9]|2[0-8]))|([0-9]{2}(0[48]|[2468][048]|[13579][26])|(0[48]|[2468][048]|[13579][26])00)-02-29)([T ]([01][0-9]|2[0-3]):[0-5][0-9](:[0-5][0-9](\.[0-9]{1,9})?)?(Z|z|[+-](0[0-9]|1[0-5])(:?[0-5][0-9])?)?)?$'
        THEN CASE WHEN value ~ ':[0-9]{2}(\.[0-9]{1,9})?(Z|z|[+-][0-9]{2}(:?[0-9]{2})?)$'
                  THEN value::timestamptz AT TIME ZONE 'UTC'
                  ELSE value::timestamp END
        END;

DROP INDEX IF EXISTS explore.place_like_score_guarded_place_only_idx;
DROP INDEX IF EXISTS explore.place_like_score_guarded_world_only_idx;

ALTER TABLE IF EXISTS explore.place
    ADD COLUMN IF NOT EXISTS like_score_num double precision GENERATED ALWAYS AS (CASE WHEN jsonb_typeof(raw->'like_score') IN ('number', 'string') AND raw->>'like_score' ~ '^-?(([0-9]{1,255}|[0-9]{100}[0-9]{1,208})(\.([0-9]{1,255}|[0-9]{100}[0-9]{1,223}))?|[0-9]{1,3}(\.[0-9]{1,17})?[eE][+-]?([0-9]{1,2}|[12][0-9]{2}|30[0-5])|[0-9](\.[0-9]{1,17})?[eE][+-]?30[6-7]|1(\.(7[0-9]?|[0-6][0-9]{0,16}))?[eE][+-]?308)$' THEN (raw->>'like_score')::float8 END) STORED,
    ADD COLUMN IF NOT EXISTS like_rate_num double precision GENERATED ALWAYS AS (CASE WHEN jsonb_typeof(raw->'like_rate') IN ('number', 'string') AND raw->>'like_rate' ~ '^-?(([0-9]{1,255}|[0-9]{100}[0-9]{1,208})(\.([0-9]{1,255}|[0-9]{100}[0-9]{1,223}))?|[0-9]{1,3}(\.[0-9]{1,17})?[eE][+-]?([0-9]{1,2}|[12][0-9]{2}|30[0-5])|[0-9](\.[0-9]{1,17})?[eE][+-]?30[6-7]|1(\.(7[0-9]?|[0-6][0-9]{0,16}))?[eE][+-]?308)$' THEN (raw->>'like_rate')::float8 END) STORED,
    ADD COLUMN IF NOT EXISTS ranking_num double precision GENERATED ALWAYS AS (CASE WHEN jsonb_typeof(raw->'ranking') IN ('number', 'string') AND raw->>'ranking' ~ '^-?(([0-9]{1,255}|[0-9]{100}[0-9]{1,208})(\.([0-9]{1,255}|[0-9]{100}[0-9]{1,223}))?|[0-9]{1,3}(\.[0-9]{1,17})?[eE][+-]?([0-9]{1,2}|[12][0-9]{2}|30[0-5])|[0-9](\.[0-9]{1,17})?[eE][+-]?30[6-7]|1(\.(7[0-9]?|[0-6][0-9]{0,16}))?[eE][+-]?308)$' THEN (raw->>'ranking')::float8 END) STORED,
    ADD COLUMN IF NOT EXISTS skybox_time_num double precision GENERATED ALWAYS AS (CASE WHEN jsonb_typeof(raw->'skybox_time') IN ('number', 'string') AND raw->>'skybox_time' ~ '^-?(([0-9]{1,255}|[0-9]{100}[0-9]{1,208})(\.([0-9]{1,255}|[0-9]{100}[0-9]{1,223}))?|[0-9]{1,3}(\.[0-9]{1,17})?[eE][+-]?([0-9]{1,2}|[12][0-9]{2}|30[0-5])|[0-9](\.[0-9]{1,17})?[eE][+-]?30[6-7]|1(\.(7[0-9]?|[0-6][0-9]{0,16}))?[eE][+-]?308)$' THEN (raw->>'skybox_time')::float8 END) STORED,
    ADD COLUMN IF NOT EXISTS user_count_num integer GENERATED ALWAYS AS (CASE WHEN jsonb_typeof(raw->'user_count') IN ('number', 'string') THEN CASE WHEN raw->>'user_count' ~ '^-?[0-9]{1,10}$' THEN CASE WHEN (raw->>'user_count')::bigint BETWEEN -2147483648 AND 2147483647 THEN (raw->>'user_count')::int END END END) STORED,
    ADD COLUMN IF NOT EXISTS user_visits_num integer GENERATED ALWAYS AS (CASE WHEN jsonb_typeof(raw->'user_visits') IN ('number', 'string') THEN CASE WHEN raw->>'user_visits' ~ '^-?[0-9]{1,10}$' THEN CASE WHEN (raw->>'user_visits')::bigint BETWEEN -2147483648 AND 2147483647 THEN (raw->>'user_visits')::int END END END) STORED,
    ADD COLUMN IF NOT EXISTS settings_version_num integer GENERATED ALWAYS AS (CASE WHEN jsonb_typeof(raw->'settings_version') IN ('number', 'string') THEN CASE WHEN raw->>'settings_version' ~ '^-?[0-9]{1,10}$' THEN CASE WHEN (raw->>'settings_version')::bigint BETWEEN -2147483648 AND 2147483647 THEN (raw->>'settings_version')::int END END END) STORED,
    ADD COLUMN IF NOT EXISTS created_at_ts timestamp without time zone GENERATED ALWAYS AS (explore.utc_timestamp_c37f040ae6c2(raw->>'created_at')) STORED,
    ADD COLUMN IF NOT EXISTS updated_at_ts timestamp without time zone GENERATED ALWAYS AS (explore.utc_timestamp_c37f040ae6c2(raw->>'updated_at')) STORED,
    ADD COLUMN IF NOT EXISTS disabled_at_ts timestamp without time zone GENERATED ALWAYS AS (explore.utc_timestamp_c37f040ae6c2(raw->>'disabled_at')) STORED,
    ADD COLUMN IF NOT EXISTS textsearch tsvector GENERATED ALWAYS AS ((setweight(to_tsvector('english', coalesce(title, '')), 'A') || setweight(to_tsvector('english', coalesce(NULLIF(btrim(raw->>'world_name'), ''), '')), 'A') || setweight(to_tsvector('english', (CASE WHEN strpos(coalesce(description, ''), '<') = 0 AND strpos(coalesce(description, ''), '>') = 0 THEN coalesce(description, '') ELSE regexp_replace(regexp_replace(regexp_replace(regexp_replace(regexp_replace(regexp_replace(coalesce(description, ''), '</?[a-zA-Z][^>]*>', '', 'g'), '</?[a-zA-Z][^>]*>', '', 'g'), '</?[a-zA-Z][^>]*>', '', 'g'), '</?[a-zA-Z][^>]*>', '', 'g'), '</?[a-zA-Z][^>]*>', '', 'g'), '[<>]', '', 'g') END)), 'B') || setweight(to_tsvector('english', coalesce(LOWER(COALESCE(raw->>'owner', creator_address)), '')), 'C'))) STORED;
ALTER TABLE IF EXISTS explore.place_world_local
    ADD COLUMN IF NOT EXISTS like_score_num double precision GENERATED ALWAYS AS (CASE WHEN jsonb_typeof(raw->'like_score') IN ('number', 'string') AND raw->>'like_score' ~ '^-?(([0-9]{1,255}|[0-9]{100}[0-9]{1,208})(\.([0-9]{1,255}|[0-9]{100}[0-9]{1,223}))?|[0-9]{1,3}(\.[0-9]{1,17})?[eE][+-]?([0-9]{1,2}|[12][0-9]{2}|30[0-5])|[0-9](\.[0-9]{1,17})?[eE][+-]?30[6-7]|1(\.(7[0-9]?|[0-6][0-9]{0,16}))?[eE][+-]?308)$' THEN (raw->>'like_score')::float8 END) STORED,
    ADD COLUMN IF NOT EXISTS like_rate_num double precision GENERATED ALWAYS AS (CASE WHEN jsonb_typeof(raw->'like_rate') IN ('number', 'string') AND raw->>'like_rate' ~ '^-?(([0-9]{1,255}|[0-9]{100}[0-9]{1,208})(\.([0-9]{1,255}|[0-9]{100}[0-9]{1,223}))?|[0-9]{1,3}(\.[0-9]{1,17})?[eE][+-]?([0-9]{1,2}|[12][0-9]{2}|30[0-5])|[0-9](\.[0-9]{1,17})?[eE][+-]?30[6-7]|1(\.(7[0-9]?|[0-6][0-9]{0,16}))?[eE][+-]?308)$' THEN (raw->>'like_rate')::float8 END) STORED,
    ADD COLUMN IF NOT EXISTS ranking_num double precision GENERATED ALWAYS AS (CASE WHEN jsonb_typeof(raw->'ranking') IN ('number', 'string') AND raw->>'ranking' ~ '^-?(([0-9]{1,255}|[0-9]{100}[0-9]{1,208})(\.([0-9]{1,255}|[0-9]{100}[0-9]{1,223}))?|[0-9]{1,3}(\.[0-9]{1,17})?[eE][+-]?([0-9]{1,2}|[12][0-9]{2}|30[0-5])|[0-9](\.[0-9]{1,17})?[eE][+-]?30[6-7]|1(\.(7[0-9]?|[0-6][0-9]{0,16}))?[eE][+-]?308)$' THEN (raw->>'ranking')::float8 END) STORED,
    ADD COLUMN IF NOT EXISTS skybox_time_num double precision GENERATED ALWAYS AS (CASE WHEN jsonb_typeof(raw->'skybox_time') IN ('number', 'string') AND raw->>'skybox_time' ~ '^-?(([0-9]{1,255}|[0-9]{100}[0-9]{1,208})(\.([0-9]{1,255}|[0-9]{100}[0-9]{1,223}))?|[0-9]{1,3}(\.[0-9]{1,17})?[eE][+-]?([0-9]{1,2}|[12][0-9]{2}|30[0-5])|[0-9](\.[0-9]{1,17})?[eE][+-]?30[6-7]|1(\.(7[0-9]?|[0-6][0-9]{0,16}))?[eE][+-]?308)$' THEN (raw->>'skybox_time')::float8 END) STORED,
    ADD COLUMN IF NOT EXISTS user_count_num integer GENERATED ALWAYS AS (CASE WHEN jsonb_typeof(raw->'user_count') IN ('number', 'string') THEN CASE WHEN raw->>'user_count' ~ '^-?[0-9]{1,10}$' THEN CASE WHEN (raw->>'user_count')::bigint BETWEEN -2147483648 AND 2147483647 THEN (raw->>'user_count')::int END END END) STORED,
    ADD COLUMN IF NOT EXISTS user_visits_num integer GENERATED ALWAYS AS (CASE WHEN jsonb_typeof(raw->'user_visits') IN ('number', 'string') THEN CASE WHEN raw->>'user_visits' ~ '^-?[0-9]{1,10}$' THEN CASE WHEN (raw->>'user_visits')::bigint BETWEEN -2147483648 AND 2147483647 THEN (raw->>'user_visits')::int END END END) STORED,
    ADD COLUMN IF NOT EXISTS settings_version_num integer GENERATED ALWAYS AS (CASE WHEN jsonb_typeof(raw->'settings_version') IN ('number', 'string') THEN CASE WHEN raw->>'settings_version' ~ '^-?[0-9]{1,10}$' THEN CASE WHEN (raw->>'settings_version')::bigint BETWEEN -2147483648 AND 2147483647 THEN (raw->>'settings_version')::int END END END) STORED,
    ADD COLUMN IF NOT EXISTS created_at_ts timestamp without time zone GENERATED ALWAYS AS (explore.utc_timestamp_c37f040ae6c2(raw->>'created_at')) STORED,
    ADD COLUMN IF NOT EXISTS updated_at_ts timestamp without time zone GENERATED ALWAYS AS (explore.utc_timestamp_c37f040ae6c2(raw->>'updated_at')) STORED,
    ADD COLUMN IF NOT EXISTS disabled_at_ts timestamp without time zone GENERATED ALWAYS AS (explore.utc_timestamp_c37f040ae6c2(raw->>'disabled_at')) STORED,
    ADD COLUMN IF NOT EXISTS textsearch tsvector GENERATED ALWAYS AS ((setweight(to_tsvector('english', coalesce(title, '')), 'A') || setweight(to_tsvector('english', coalesce(world_name, '')), 'A') || setweight(to_tsvector('english', (CASE WHEN strpos(coalesce(description, ''), '<') = 0 AND strpos(coalesce(description, ''), '>') = 0 THEN coalesce(description, '') ELSE regexp_replace(regexp_replace(regexp_replace(regexp_replace(regexp_replace(regexp_replace(coalesce(description, ''), '</?[a-zA-Z][^>]*>', '', 'g'), '</?[a-zA-Z][^>]*>', '', 'g'), '</?[a-zA-Z][^>]*>', '', 'g'), '</?[a-zA-Z][^>]*>', '', 'g'), '</?[a-zA-Z][^>]*>', '', 'g'), '[<>]', '', 'g') END)), 'B') || setweight(to_tsvector('english', coalesce(LOWER(COALESCE(raw->>'owner', creator_address)), '')), 'C'))) STORED;

-- The recurrence instants of an event: each `raw.recurrent_dates` entry that
-- is an ISO 8601 date-time (the utc_timestamp function), ascending. The lists
-- expand this array (events/mod.rs VALID_RECURRENCE_DATES_SQL).
CREATE FUNCTION explore.recurrence_dates_17e91951d5fe(raw jsonb) RETURNS timestamp without time zone[]
    LANGUAGE sql IMMUTABLE PARALLEL SAFE STRICT
    RETURN ARRAY(SELECT parsed.d FROM (SELECT explore.utc_timestamp_c37f040ae6c2(value) AS d
                 FROM jsonb_array_elements_text(CASE WHEN jsonb_typeof(raw->'recurrent_dates') = 'array'
                                                     THEN raw->'recurrent_dates' ELSE '[]'::jsonb END)) parsed
                 WHERE parsed.d IS NOT NULL ORDER BY parsed.d);

-- The last instant an event is live under the lists' effective occurrence
-- (events/mod.rs EFF_NEXT_FINISH_SQL): the effective finish is later than a
-- moment exactly when this is, so a live list reads an index range instead
-- of expanding every row's recurrence. NULL when the row has no instant.
CREATE FUNCTION explore.event_live_until_8c77b09c74f5(next_start_at timestamp without time zone, start_at timestamp without time zone, duration_ms bigint, raw jsonb)
    RETURNS timestamp without time zone
    LANGUAGE sql IMMUTABLE PARALLEL SAFE
    RETURN GREATEST(
        next_start_at + COALESCE(duration_ms, 0) * interval '1 millisecond',
        (SELECT max(d) FROM unnest(explore.recurrence_dates_17e91951d5fe(raw)) d)
            + COALESCE(duration_ms, 0) * interval '1 millisecond',
        CASE WHEN next_start_at IS NULL AND cardinality(explore.recurrence_dates_17e91951d5fe(raw)) = 0
             THEN start_at + COALESCE(duration_ms, 0) * interval '1 millisecond' END);

ALTER TABLE IF EXISTS explore.event
    ADD COLUMN IF NOT EXISTS rejected boolean NOT NULL GENERATED ALWAYS AS (COALESCE(raw->'rejected' = 'true'::jsonb, false)) STORED,
    ADD COLUMN IF NOT EXISTS deleted boolean NOT NULL GENERATED ALWAYS AS (COALESCE(raw->>'deleted_by_user' = 'true', false) OR COALESCE(raw->>'deleted_by_admin' = 'true', false)) STORED,
    ADD COLUMN IF NOT EXISTS world boolean NOT NULL GENERATED ALWAYS AS (COALESCE(raw->'world' = 'true'::jsonb, false)) STORED,
    ADD COLUMN IF NOT EXISTS place_id_lower text GENERATED ALWAYS AS (lower(raw->>'place_id')) STORED,
    ADD COLUMN IF NOT EXISTS server_lower text GENERATED ALWAYS AS (lower(raw->>'server')) STORED,
    ADD COLUMN IF NOT EXISTS created_at_ts timestamp without time zone GENERATED ALWAYS AS (explore.utc_timestamp_c37f040ae6c2(raw->>'created_at')) STORED,
    ADD COLUMN IF NOT EXISTS recurrence_dates timestamp without time zone[] NOT NULL GENERATED ALWAYS AS (explore.recurrence_dates_17e91951d5fe(raw)) STORED,
    ADD COLUMN IF NOT EXISTS live_until timestamp without time zone GENERATED ALWAYS AS (explore.event_live_until_8c77b09c74f5(next_start_at, start_at, duration_ms, raw)) STORED,
    ADD COLUMN IF NOT EXISTS textsearch tsvector NOT NULL GENERATED ALWAYS AS ((setweight(to_tsvector('english', coalesce(name,'')), 'A') || setweight(to_tsvector('english', coalesce(raw->>'user_name','')), 'B') || setweight(to_tsvector('english', coalesce(raw->>'estate_name','')), 'B') || setweight(to_tsvector('english', coalesce(description,'')), 'C'))) STORED;
