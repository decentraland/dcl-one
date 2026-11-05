-- Existing communities start tracking at migration time. The canonical object
-- pass creates the lifetime receipts and pending-photo partial index declared
-- in the fresh bootstrap; this step only extends existing tables.
ALTER TABLE social.communities ADD COLUMN IF NOT EXISTS ranking_events_count integer NOT NULL DEFAULT 0;
ALTER TABLE social.communities ADD COLUMN IF NOT EXISTS ranking_photos_count integer NOT NULL DEFAULT 0;
ALTER TABLE social.communities ADD COLUMN IF NOT EXISTS ranking_streams_count integer NOT NULL DEFAULT 0;
ALTER TABLE social.communities ADD COLUMN IF NOT EXISTS ranking_events_total_attendees integer NOT NULL DEFAULT 0;
ALTER TABLE social.communities ADD COLUMN IF NOT EXISTS ranking_streams_total_participants integer NOT NULL DEFAULT 0;
ALTER TABLE social.communities ADD COLUMN IF NOT EXISTS ranking_tracking_since timestamp without time zone NOT NULL DEFAULT timezone('UTC', now());
ALTER TABLE social.communities ADD CONSTRAINT communities_ranking_nonnegative CHECK (ranking_events_count >= 0 AND ranking_photos_count >= 0 AND ranking_streams_count >= 0 AND ranking_events_total_attendees >= 0 AND ranking_streams_total_participants >= 0);
ALTER TABLE camera_reel.images ADD COLUMN IF NOT EXISTS ranking_published boolean NOT NULL DEFAULT true;
