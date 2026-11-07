-- Places moderation: the content-rating audit log and the moderation
-- notification outbox (src/explore/places/moderation.rs).

-- Append-only: one row per rating decision on a place or a world, the old
-- service's `content_ratings` table (places `PlaceContentRating/model.ts`,
-- written by `Place/routes/updateRating.ts:60-68` and
-- `World/routes/updateWorldRating.ts:71-79`). Written in the decision's
-- transaction whether or not the catalogue row accepted the write (upstream
-- inserted it beside an `UPDATE` that could match nothing). Workload: a
-- handful of operator decisions a day, read by the console's per-entity
-- history; never updated or deleted. `entity_type` tells a world's name
-- from a place's uuid. No FK to the catalogue: the history outlives a row
-- the sweep drops. `update_rating` is the stored code (`RP` for pending).
CREATE TABLE explore.place_content_ratings (
    id              uuid PRIMARY KEY,
    entity_id       text NOT NULL,
    entity_type     text NOT NULL,
    original_rating text,
    update_rating   text NOT NULL,
    moderator       text NOT NULL,
    comment         text,
    created_at      timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT place_content_ratings_entity_type_known CHECK (entity_type IN ('place', 'world')),
    CONSTRAINT place_content_ratings_update_known CHECK (update_rating IN ('RP', 'E', 'T', 'A', 'R'))
);
-- The history of one entity, newest first: `WHERE entity_id = $1 ORDER BY created_at DESC, id`.
CREATE INDEX place_content_ratings_entity_idx ON explore.place_content_ratings (entity_id, created_at DESC, id);
