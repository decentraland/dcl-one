-- Only unchanged imported rows are eligible for upstream cancellation.
DO $empty$
BEGIN
  IF NOT EXISTS (SELECT 1 FROM stage)
     AND current_setting('dcl_migration.allow_empty', true) IS DISTINCT FROM 'on'
     AND EXISTS (SELECT 1 FROM dcl_migration.event_attendance_loaded) THEN
    RAISE EXCEPTION 'events/attendance_reconcile: upstream lists no attendees but imported ones are recorded; use --allow-empty events/attendance_reconcile only after verifying the source';
  END IF;
END $empty$;

CREATE TEMP TABLE attendance_gone ON COMMIT DROP AS
SELECT l.* FROM dcl_migration.event_attendance_loaded l
WHERE NOT EXISTS (SELECT 1 FROM stage s WHERE s.event_id=l.event_id AND s.signer=l.signer);

DELETE FROM explore.event_attendance_local a USING attendance_gone g
WHERE a.event_id=g.event_id AND a.signer=g.signer AND a.action='going'
  AND a.signed_at=g.signed_at AND a.signed_payload=g.signed_payload;

DELETE FROM dcl_migration.event_attendance_loaded l USING attendance_gone g
WHERE l.event_id=g.event_id AND l.signer=g.signer;
