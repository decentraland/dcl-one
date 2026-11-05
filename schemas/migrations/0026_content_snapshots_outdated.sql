-- The deleter repair marks a snapshot window for regeneration with this flag
-- instead of writing the epoch over its generation time, so /snapshots keeps
-- publishing the real generationTimestamp until the next pass rebuilds it.
-- A constant default: no table rewrite. Windows already stamped at the epoch
-- carry the old mark.
ALTER TABLE content.snapshots ADD COLUMN IF NOT EXISTS outdated boolean NOT NULL DEFAULT false;
UPDATE content.snapshots SET outdated = true WHERE generation_time = 'epoch'::timestamp AND NOT outdated;
