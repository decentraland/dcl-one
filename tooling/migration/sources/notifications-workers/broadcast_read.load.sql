-- Seed parents as well so this step can be resumed or run independently.

-- ── ingest id (identical in the notification, broadcast_read and broadcast loads) ──
-- governance_/reward_ rows take ingest.rs `Ingested::id` (v5 in INGEST_NS over the compact
-- `[type,address,eventKey]`) so a re-post updates them; others keep `fallback`. No pgcrypto.
CREATE OR REPLACE FUNCTION pg_temp.nw_sha1(msg bytea) RETURNS bytea
LANGUAGE plpgsql IMMUTABLE STRICT AS $f$
DECLARE
  mask CONSTANT bigint := 4294967295;
  m bytea := msg || '\x80'::bytea || decode(repeat('00', (119 - length(msg) % 64) % 64), 'hex')
             || int8send(length(msg)::bigint * 8);
  h0 bigint := 1732584193; h1 bigint := 4023233417; h2 bigint := 2562383102;
  h3 bigint := 271733878; h4 bigint := 3285377520;
  a bigint; b bigint; c bigint; d bigint; e bigint; f bigint; k bigint; x bigint;
  w bigint[];
  o int;
BEGIN
  FOR blk IN 0 .. length(m) / 64 - 1 LOOP
    o := blk * 64;
    w := '{}';
    FOR t IN 0 .. 15 LOOP
      w[t] := (get_byte(m, o + 4 * t)::bigint << 24) | (get_byte(m, o + 4 * t + 1)::bigint << 16)
              | (get_byte(m, o + 4 * t + 2)::bigint << 8) | get_byte(m, o + 4 * t + 3)::bigint;
    END LOOP;
    FOR t IN 16 .. 79 LOOP
      x := w[t - 3] # w[t - 8] # w[t - 14] # w[t - 16];
      w[t] := ((x << 1) | (x >> 31)) & mask;
    END LOOP;
    a := h0; b := h1; c := h2; d := h3; e := h4;
    FOR t IN 0 .. 79 LOOP
      IF t < 20 THEN f := (b & c) | ((b # mask) & d); k := 1518500249;
      ELSIF t < 40 THEN f := b # c # d; k := 1859775393;
      ELSIF t < 60 THEN f := (b & c) | (b & d) | (c & d); k := 2400959708;
      ELSE f := b # c # d; k := 3395469782;
      END IF;
      x := ((((a << 5) | (a >> 27)) & mask) + f + e + k + w[t]) & mask;
      e := d; d := c; c := ((b << 30) | (b >> 2)) & mask; b := a; a := x;
    END LOOP;
    h0 := (h0 + a) & mask; h1 := (h1 + b) & mask; h2 := (h2 + c) & mask;
    h3 := (h3 + d) & mask; h4 := (h4 + e) & mask;
  END LOOP;
  RETURN substr(int8send(h0), 5) || substr(int8send(h1), 5) || substr(int8send(h2), 5)
         || substr(int8send(h3), 5) || substr(int8send(h4), 5);
END $f$;

CREATE OR REPLACE FUNCTION pg_temp.nw_ingest_id(kind text, address text, event_key text, fallback uuid)
RETURNS uuid LANGUAGE sql IMMUTABLE AS $f$
  SELECT CASE
    WHEN (starts_with(kind, 'governance_') OR starts_with(kind, 'reward_')) AND event_key <> '' THEN (
      SELECT encode(set_byte(set_byte(substr(h, 1, 16), 6, (get_byte(h, 6) & 15) | 80),
                             8, (get_byte(h, 8) & 63) | 128), 'hex')::uuid
      FROM (SELECT pg_temp.nw_sha1('\x3f8a52d49c1e4b07a62d71e05c9318bb'::bytea
             || convert_to('[' || to_json(kind)::text || ',' || coalesce(to_json(address)::text, 'null')
                           || ',' || to_json(event_key)::text || ']', 'UTF8')) AS h) d)
    ELSE fallback END
$f$;
-- ── end ingest id ──

CREATE TEMP TABLE nw_broadcast_id ON COMMIT DROP AS
SELECT notification_id, pg_temp.nw_ingest_id(type, NULL, event_key, notification_id) AS id
FROM (SELECT DISTINCT notification_id, type, event_key FROM stage) s;

INSERT INTO social.broadcast_notifications (id, type, metadata, occurred_at, created_at)
SELECT DISTINCT ON (m.id) m.id, s.type, s.metadata,
       s.occurred_at AT TIME ZONE 'UTC', s.created_at AT TIME ZONE 'UTC'
FROM stage s JOIN nw_broadcast_id m USING (notification_id)
ORDER BY m.id, s.notification_id, s.source_address
ON CONFLICT (id) DO NOTHING;

INSERT INTO social.broadcast_reads AS t (broadcast_id, address, read_at)
SELECT m.id, s.address, min(s.read_at) AT TIME ZONE 'UTC'
FROM stage s JOIN nw_broadcast_id m USING (notification_id)
GROUP BY m.id, s.address
ON CONFLICT (broadcast_id, address) DO UPDATE SET read_at = LEAST(t.read_at, EXCLUDED.read_at);
