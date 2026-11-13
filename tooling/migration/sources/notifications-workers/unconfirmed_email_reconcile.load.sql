-- `stage` is the whole upstream set (a `whole` step). A pending email this
-- source imported and upstream no longer holds (confirmed or replaced
-- upstream) goes, unless the node has issued its own code since.
DO $empty$
BEGIN
  IF NOT EXISTS (SELECT 1 FROM stage)
     AND EXISTS (SELECT 1 FROM dcl_migration.unconfirmed_email_loaded) THEN
    RAISE EXCEPTION 'notifications-workers/unconfirmed_email_reconcile: upstream lists no pending emails but imported ones are recorded; refusing to delete them all (wrong source database?)';
  END IF;
END $empty$;

CREATE TEMP TABLE unconfirmed_email_gone ON COMMIT DROP AS
SELECT l.address, l.code
FROM dcl_migration.unconfirmed_email_loaded l
WHERE NOT EXISTS (SELECT 1 FROM stage s WHERE s.address = l.address);

DELETE FROM social.unconfirmed_emails u
USING unconfirmed_email_gone g
WHERE u.address = g.address AND u.code = g.code;

DELETE FROM dcl_migration.unconfirmed_email_loaded l
USING unconfirmed_email_gone g
WHERE l.address = g.address;

DROP TABLE unconfirmed_email_gone;
