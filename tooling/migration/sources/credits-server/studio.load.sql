-- Studios keep upstream's id, so the operators and budget entries that name a studio land on it.
-- Names tell studios apart case-insensitively on both sides: a name the node already gives another
-- studio stops the run, since which studio keeps it is an operator's decision.
DO $names$
DECLARE
  clash text;
BEGIN
  SELECT s.name INTO clash
  FROM stage s
  JOIN credits.studios n ON lower(n.name) = lower(s.name) AND n.id <> s.id
  LIMIT 1;
  IF clash IS NOT NULL THEN
    RAISE EXCEPTION 'credits-server/studio: the node already has another studio named %; rename one before importing', clash;
  END IF;
END $names$;

-- A studio the node renamed or paused after upstream's last change keeps the node's values.
INSERT INTO credits.studios AS t (id, name, status, created_by, created_at, updated_at)
SELECT s.id, s.name, s.status, s.created_by, s.created_at AT TIME ZONE 'UTC', s.updated_at AT TIME ZONE 'UTC'
FROM stage s
ON CONFLICT (id) DO UPDATE SET
    name = EXCLUDED.name,
    status = EXCLUDED.status,
    updated_at = EXCLUDED.updated_at
WHERE t.updated_at <= EXCLUDED.updated_at
  AND (t.name, t.status, t.updated_at) IS DISTINCT FROM (EXCLUDED.name, EXCLUDED.status, EXCLUDED.updated_at);
