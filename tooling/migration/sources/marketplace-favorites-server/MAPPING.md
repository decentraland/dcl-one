# favorites database → shop lists

Source: the favorites database, schema `favorites`. It was created by the
archived marketplace-favorites-server (`e25017ed`). It is now migrated and
written by marketplace-server `cc5baede` (`src/migrations/favorites`, with the
same nine files byte for byte).

| Step | Upstream | Target |
| --- | --- | --- |
| `lists` | `lists`, with its `acl` rows inlined as `grants` | `shop.lists`, `shop.list_acl` |
| `picks` | `picks` | `shop.picks` |

## Rules

- Ids are kept. Owners, grantees and pick keys are lowercased, as the node
  writes them. A pick duplicated only by case keeps its earliest `created_at`.
- `is_private` is stored the way the node's writes store it: true unless the
  list has a `view` grant to `*`. Reads derive it from the ACL anyway.
  `permission` is NULL (the node never writes it).
- A list's grants are replaced by the source's grants each time the list loads.
  Upstream triggers bump `lists.updated_at` (the watermark) on every grant or
  pick change, so `--since` sees revocations. Picks deleted upstream after a
  run are not removed.
- The shared default list `70ab6873-...` is the node's seed and is not
  overwritten (upstream leaves it without a `*` grant). Picks into it move.
- If the node already has a list with the same `(name, owner)` under another id,
  the node's list is kept. The upstream list, its grants and its picks are
  skipped.
- `timestamp` columns hold UTC. A NULL `updated_at` stays NULL; the watermark
  falls back to `created_at`.

## Not migrated

`voting`: cached snapshot voting power; the node parses `power` but does not
apply it. `pgmigrations`.
