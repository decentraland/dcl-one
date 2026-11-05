-- The collections subgraphs' `Item.searchText`, which lamb2's
-- `/collections/{wearables,emotes}?textSearch=` matches with
-- `searchText_contains`. Nullable with no default: no table rewrite; the
-- items mirror fills it on its next pass and the catalog reads
-- `lower(name)` until then.
ALTER TABLE shop.items ADD COLUMN IF NOT EXISTS search_text text;
