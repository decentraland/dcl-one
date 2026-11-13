-- `stage` is every post id upstream holds (a `whole` step). A post this
-- source wrote and upstream deleted goes, unless the node edited it since;
-- its likes go with it.
DO $empty$
BEGIN
  IF NOT EXISTS (SELECT 1 FROM stage)
     AND EXISTS (SELECT 1 FROM dcl_migration.social_community_post_loaded) THEN
    RAISE EXCEPTION 'social-service-ea/community_posts_reconcile: upstream lists no posts but imported ones are recorded; refusing to delete them all (wrong source database?)';
  END IF;
END $empty$;

CREATE TEMP TABLE community_post_gone ON COMMIT DROP AS
SELECT l.id, l.content
FROM dcl_migration.social_community_post_loaded l
WHERE NOT EXISTS (SELECT 1 FROM stage s WHERE s.id = l.id);

DELETE FROM social.community_posts p
USING community_post_gone g
WHERE p.id = g.id AND p.content = g.content;

DELETE FROM dcl_migration.social_community_post_loaded l
USING community_post_gone g
WHERE l.id = g.id;

DROP TABLE community_post_gone;
