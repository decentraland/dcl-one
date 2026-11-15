-- No delivery rows: imported history is never e-mailed again. Read state
-- only moves forward, so a read on the node survives a re-import.

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

CREATE TEMP TABLE nw_notification ON COMMIT DROP AS
SELECT s.id AS source_id, pg_temp.nw_ingest_id(s.type, s.address, s.event_key, s.id) AS id,
       s.address, s.type, s.metadata, s.occurred_at, s.read_at, s.created_at, s.updated_at
FROM stage s;

-- An earlier import kept the upstream id: carry its read over, then drop it.
UPDATE nw_notification n SET read_at = least(n.read_at, o.read_at)
FROM social.notifications o
WHERE o.id = n.source_id AND n.id <> n.source_id AND o.address = n.address AND o.type = n.type AND o.read;

DELETE FROM social.notifications o USING nw_notification n
WHERE o.id = n.source_id AND n.id <> n.source_id AND o.address = n.address AND o.type = n.type;

-- Upstream rows whose addresses differ only in case share an ingest id: the newest wins, the earliest read stays.
INSERT INTO social.notifications
    (id, address, type, metadata, occurred_at, read, read_at, created_at, in_app)
SELECT DISTINCT ON (id) id, address, type, metadata, occurred_at,
       first_read IS NOT NULL, first_read, created_at, true
FROM (SELECT n.*, min(n.read_at) OVER (PARTITION BY n.id) AS first_read FROM nw_notification n) n
ORDER BY id, updated_at DESC, source_id
ON CONFLICT (id) DO UPDATE
   SET type = EXCLUDED.type,
       metadata = EXCLUDED.metadata,
       occurred_at = EXCLUDED.occurred_at,
       read = social.notifications.read OR EXCLUDED.read,
       read_at = CASE WHEN social.notifications.read
                      THEN social.notifications.read_at
                      ELSE EXCLUDED.read_at END;
