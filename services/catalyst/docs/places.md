# Places and worlds: like rates, moderation notices and browser origins

The places routes answer the old places service (`decentraland/places@e660fbc2`) for the Explorer and its website; `tests/fixtures/explore_parity/README.md` describes the fixtures that pin them. This page covers what a node operator configures or runs for them.

## Like rates

A place or world carries `likes`, `dislikes`, `like_rate` and `like_score` (the Wilson lower bound over the voters whose activity reaches 100). The mirror brings the old places service's aggregates, not its voters. A vote on this node is a fact in `explore.user_likes` with the wallet's voting power at vote time (asked of the Snapshot score API `SNAPSHOT_SCORE_URL` names; unset, every vote carries 0 and none qualifies), and the served rate is computed over the old service's qualified votes reconstructed from the mirrored aggregates plus this node's; it is stored on `explore.place_interaction_counts` once a local vote qualifies. In that reconstruction a wallet that voted there and here counts twice.

The old places service's own vote rows are the ground truth. Export them from the old places database and import them once:

```sh
psql "$OLD_PLACES_DATABASE_URL" -Atc "COPY (SELECT row_to_json(l) FROM user_likes l) TO STDOUT" > user_likes.jsonl
catalyst explore import-likes --database-url "$DATABASE_URL" --file user_likes.jsonl
```

Each row becomes a fact marked as imported in its `source` column, with its voting power at vote time (an export names the entity `entity_id` or `place_id`; both are read). A wallet that already voted here keeps its local fact, so nobody counts twice, and a later local vote replaces the imported one. Each entity is settled and recounted in its own transaction: a vote on a settled entity waits for that entity alone, and a stopped import keeps what it settled. A malformed line imports nothing and names itself. The import settles an entity once; a fresher export adds nothing to a settled entity, so export the rows after the old service stops taking votes. An entity the import covered takes its likes, dislikes, like rate and like score from the fact table alone, even after its imported voters withdraw; the mirrored totals, which count the same voters, stay out of its answer. The command prints the outcome as JSON (`read`, `imported`, `kept_local`, `settled`, `entities`); a later import leaves an entity it already covered alone, so importing the same export again changes nothing and a vote a wallet withdrew here stays withdrawn. Run the import before the likes routes of this node take votes: until the import covers an entity, a vote by a wallet the export already counts adds to the mirrored totals, a vote that wallet withdrew here returns with the export, and a vote re-cast with its exported value keeps the fresh voting power where the old service kept the stored one.

## Moderation notices

A disable or a rating upgrade posts a Slack-compatible notice to `PLACES_MODERATION_WEBHOOK`; the events notices post to `SLACK_WEBHOOK` ([events](events.md)). Both are captured in the writing transaction as rows of one outbox, `explore.moderation_hooks` (`channel` is `events` or `places`), and posted by the `moderation-hook` job every minute: six attempts with exponential backoff, rows kept thirty days, nothing posted before the node's first capture. Both need `OPS_WRITES_ENABLED`. While the database has the table `explore.event_moderation_hooks`, every pass of the job first moves its pending rows into the outbox ([events](events.md)).

## Content ratings

`PUT /api/places/{id}/rating` and `PUT /api/worlds/{id}/rating` take the old places service's rating codes. The stored code `RP`, which its PUT schema refuses, is accepted as well as `PR`; the code is answered verbatim. A disabled genesis place takes no rating or highlight, as the old service's update matched no such row; the rating audit row is written either way, and the route echoes the sent code as the old one did.

## Browser origins

The `/api` routes answer CORS for the old services' origin lists (decentraland.org and its subdomains, the places and events sites, local development ports). `EXPLORE_CORS_ORIGINS` appends patterns to every list: comma-separated regular expressions matched against the `Origin` header, for example `^https://explore\.example\.org$`. Each entry is a Rust regular expression anchored with `^` and `$`; `\d` means ASCII digits, and lookaheads and backreferences are not accepted. An entry without both anchors, or one that does not compile, refuses to start.

## Dates

Place and world records carry their dates as node's `Date#toJSON` writes them (`2026-01-01T00:00:00.000Z`), like the old service.

## Stored read columns

Both catalogue legs (`explore.place`, `explore.place_world_local`) store the guarded reads of the raw document that the lists order, filter and project: `like_score_num`, `like_rate_num`, `ranking_num`, `skybox_time_num`, `user_count_num`, `user_visits_num`, `settings_version_num`, `created_at_ts`, `updated_at_ts`, `disabled_at_ts` and the `textsearch` vector. They are generated columns, so the mirror and the local writers do not write them; a row pays the parse once at write time and a page comes from an index (`place_like_score_*_idx`, `place_textsearch_gin`, `place_owner_lower_idx` and their local-leg twins) instead of parsing every row. the hash-named `utc_timestamp` function holds the date guard; `query.rs` pins every expression against the schema. A canonical world scene, which is not a table row, computes the same columns in its SELECT (`typed_columns_sql`). A list sorts, counts and cuts its keys in a `page` CTE and reads the columns of the page alone; the `like_score` order sorts each leg on the score the row answers, the recount's over the mirrored one, reading the sort keys from the leg's partial index.
