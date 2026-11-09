-- The recurrence columns of explore.event (step 0030: recurrence_dates,
-- live_until) parse every `raw.recurrent_dates` entry on every write of the
-- row. The shipped bodies called the utc_timestamp function per entry (a
-- SQL function a generated column's expression cannot inline) and live_until
-- expanded the array twice more, three guarded parses per entry; a write
-- cost 21 us per entry (a mirror pass over 7 686 events 290 ms of its 1.4 s,
-- the social suite's 10 000 x 100 seed 20 s, past the pool's 60 s statement
-- timeout on a shared runner). These bodies carry the guard inline and read
-- each entry once per column, live_until through one bounded aggregate:
-- 5.7 us per entry, the seed 5.4 s. The values are the ones the shipped
-- bodies computed (schema_functions.rs pins the replacement to the binary's
-- definition and a test compares both on every shape of document), so the
-- shipped names keep their bodies replaced in place: no column changes, no
-- row is rewritten, and the stored columns stay right. A fresh database
-- creates the same bodies under the names the binary hashes
-- (schemas/10_explore.sql); the generated columns of a migrated database
-- keep calling the names below, so `catalyst schema status` never lists
-- them as superseded (a function a column depends on is kept).
CREATE OR REPLACE FUNCTION explore.recurrence_dates_17e91951d5fe(raw jsonb) RETURNS timestamp without time zone[]
    LANGUAGE sql IMMUTABLE PARALLEL SAFE STRICT
    RETURN ARRAY(WITH parsed AS MATERIALIZED (
                   SELECT CASE WHEN value ~ '^((000[1-9]|00[1-9][0-9]|0[1-9][0-9]{2}|[1-9][0-9]{3})-((0[13578]|1[02])-(0[1-9]|[12][0-9]|3[01])|(0[469]|11)-(0[1-9]|[12][0-9]|30)|02-(0[1-9]|1[0-9]|2[0-8]))|([0-9]{2}(0[48]|[2468][048]|[13579][26])|(0[48]|[2468][048]|[13579][26])00)-02-29)([T ]([01][0-9]|2[0-3]):[0-5][0-9](:[0-5][0-9](\.[0-9]{1,9})?)?(Z|z|[+-](0[0-9]|1[0-5])(:?[0-5][0-9])?)?)?$'
                       THEN CASE WHEN value ~ ':[0-9]{2}(\.[0-9]{1,9})?(Z|z|[+-][0-9]{2}(:?[0-9]{2})?)$'
                                 THEN value::timestamptz AT TIME ZONE 'UTC'
                                 ELSE value::timestamp END
                       END AS d
                   FROM jsonb_array_elements_text(CASE WHEN jsonb_typeof(raw->'recurrent_dates') = 'array'
                                                      THEN raw->'recurrent_dates' ELSE '[]'::jsonb END))
                 SELECT d FROM parsed WHERE d IS NOT NULL ORDER BY d);

CREATE OR REPLACE FUNCTION explore.event_live_until_8c77b09c74f5(next_start_at timestamp without time zone, start_at timestamp without time zone, duration_ms bigint, raw jsonb)
    RETURNS timestamp without time zone
    LANGUAGE sql IMMUTABLE PARALLEL SAFE
    RETURN (SELECT GREATEST(
        next_start_at + COALESCE(duration_ms, 0) * interval '1 millisecond',
        last.d + COALESCE(duration_ms, 0) * interval '1 millisecond',
        CASE WHEN next_start_at IS NULL AND last.d IS NULL
             THEN start_at + COALESCE(duration_ms, 0) * interval '1 millisecond' END)
      FROM (SELECT max(CASE WHEN value ~ '^((000[1-9]|00[1-9][0-9]|0[1-9][0-9]{2}|[1-9][0-9]{3})-((0[13578]|1[02])-(0[1-9]|[12][0-9]|3[01])|(0[469]|11)-(0[1-9]|[12][0-9]|30)|02-(0[1-9]|1[0-9]|2[0-8]))|([0-9]{2}(0[48]|[2468][048]|[13579][26])|(0[48]|[2468][048]|[13579][26])00)-02-29)([T ]([01][0-9]|2[0-3]):[0-5][0-9](:[0-5][0-9](\.[0-9]{1,9})?)?(Z|z|[+-](0[0-9]|1[0-5])(:?[0-5][0-9])?)?)?$'
                    THEN CASE WHEN value ~ ':[0-9]{2}(\.[0-9]{1,9})?(Z|z|[+-][0-9]{2}(:?[0-9]{2})?)$'
                              THEN value::timestamptz AT TIME ZONE 'UTC'
                              ELSE value::timestamp END
                    END) AS d
            FROM jsonb_array_elements_text(CASE WHEN jsonb_typeof(raw->'recurrent_dates') = 'array'
                                               THEN raw->'recurrent_dates' ELSE '[]'::jsonb END)) last);
