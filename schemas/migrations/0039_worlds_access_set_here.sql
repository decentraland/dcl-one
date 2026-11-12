-- Whether the owner set a world's access on this node, so the worlds mirror
-- never replaces it with its source's (core::store::replace_mirrored_access).
-- A constant default adds the column without rewriting the table. Before this
-- step the mirror copied no access setting, so a stored setting other than
-- the default was written by the owner here and is marked as such.
ALTER TABLE IF EXISTS worlds.worlds ADD COLUMN IF NOT EXISTS access_set_here boolean NOT NULL DEFAULT false;
UPDATE worlds.worlds SET access_set_here = true WHERE access_type <> 'unrestricted' AND NOT access_set_here;
