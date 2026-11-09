# Database migration from the TypeScript services

`migrate.py` copies rows from the Postgres databases of the Decentraland
TypeScript services into a `catalyst` database. It is one engine driven by one
manifest per source service under `sources/<name>/`: the engine owns batching,
checkpoints, dry runs and `--since`; each manifest owns its SQL mapping and a
`MAPPING.md` saying what is and is not carried over. One engine keeps resume,
dry-run and re-import behavior identical for every source; per-source scripts
would each re-implement them.

It reads from the sources (sessions are read-only) and writes only to the
target. It needs `psql` (PostgreSQL 15 or later) and Python 3.11 or later
(`tomllib`), nothing else; `blobs --from-s3` also needs `boto3`.

Out of scope: the Catalyst content servers (`content.*` comes from peer sync),
transactions-server (its relay history has no chain or contract, which
`economy.transactions` requires), chain-derived tables the node's indexers
rebuild, and rows the upstream mirrors fill. Each `MAPPING.md` names the
tables it leaves to the node. `blobs` copies the blobs of the sources it knows
(`worlds-content-server`), and `blobs badges` the badge art into the node's
`BADGES_ASSETS_DIR`; the others need an external copy that fills the same
records.

Known limitations after a cutover: MANA-denominated credits stay spendable
on-chain until they expire but are in no node balance, and credits from Apple
in-app purchases cannot be clawed back by a later Apple refund
(`sources/credits-server/MAPPING.md`).

## Before a cutover run

- **Blobs first.** Rows that name stored files (builder item contents, camera
  reel photos, world scene entities, content files and thumbnails) need those
  files in the blob store. The blob copy records every object it stores as
  `(source, object_key, hash, size)` in `dcl_migration.blob_inventory`, and
  every object it could not store in `dcl_migration.blob_missing`; loads that
  need a blob stop the batch when it is in neither. `migrate.py blobs <source>`
  is that copy for the sources it supports (see `blobs` below). **Do not run
  the node's storage GC between `blobs` and `run`**: the copied blobs are
  unreferenced until `run` imports their rows, and the GC deletes unreferenced
  files older than an hour. A `run` of a source `blobs` covers ends by
  checking the store holds every blob its imported rows name, and fails when
  one is missing. `blobs worlds-content-server` also goes before `run places`
  (world images are rewritten to the copied thumbnails' CIDs).
- **Badge art.** The node embeds the art of its seed badges only. After
  `run badges`, `migrate.py blobs badges --to "$BADGES_ASSETS_DIR"` copies
  every texture the imported catalog's `assets` name from upstream's CDN (or
  `--from-dir`, a local copy laid out by path) into the node's badge art
  directory; `--verify` lists the ones it lacks (`sources/badges/MAPPING.md`).
- **Badge schema.** The badges steps need the badge catalog schema
  (publication columns, `criteria_steps`, the progress document,
  `social.presence_badges`) and stop without it.
- **Mirrors off.** Turn off the node's mirrors of a migrated service
  (`PLACES_MIRROR_UPSTREAM`, `EVENTS_MIRROR_UPSTREAM`,
  `COMMUNITIES_MIRROR_UPSTREAM`, `WORLDS_MIRROR_UPSTREAM`, ...) before its
  import, or the next mirror pass turns imported rows back into mirrored ones.
  The per-source `MAPPING.md` says which mirror applies.
- **Freeze upstream writes for the final pass.** Re-runs do not see upstream
  deletes (except the `whole` reconcile steps a manifest declares), so the
  last `run --since last` happens after the old service stops writing.
- **Same secrets.** Values the upstream encrypted (world env variables) are
  copied as ciphertext; the node must run with the upstream key.

## Usage

```sh
export TARGET_DATABASE_URL=postgres://...      # the catalyst database
export PLACES_DATABASE_URL=postgres://...      # one variable per source, see `list`

python3 tooling/migration/migrate.py list                 # sources, steps, variables
python3 tooling/migration/migrate.py check places         # validate manifests and SQL, no writes
python3 tooling/migration/migrate.py run places --dry-run # load everything, then roll back
python3 tooling/migration/migrate.py run places           # migrate, resuming where it stopped
python3 tooling/migration/migrate.py run all              # every source, in dependency order
python3 tooling/migration/migrate.py status               # checkpoints per step
python3 tooling/migration/migrate.py run places --since 2026-10-01T00:00:00Z
python3 tooling/migration/migrate.py run places --since last
python3 tooling/migration/migrate.py blobs worlds-content-server --from-dir DIR --to "$STORAGE_ROOT_FOLDER"
python3 tooling/migration/migrate.py blobs worlds-content-server --verify --to "$STORAGE_ROOT_FOLDER"
python3 tooling/migration/migrate.py blobs badges --to "$BADGES_ASSETS_DIR"   # after `run badges`
python3 tooling/migration/migrate.py worlds-limits whitelist.json --max-size-mb 100
python3 tooling/migration/migrate.py worlds-unban-quota-blocks --dry-run   # one-off, see the worlds MAPPING.md
```

`blobs <source>` reads from the source database which objects its rows name,
copies each from upstream storage (`--from-dir`, or `--from-s3 BUCKET`) into
the node's store under `--to` (`<root>/contents/<sha1 shard>/<cid>`, staging
file then rename), after re-hashing the bytes against their key; it skips
objects already recorded and stored and those recorded missing (`--retry-missing`
retries them), and fails before recording anything missing when the source
looks wrong, so it is re-run before each `run --since last`. `--verify` lists the imported rows whose blobs the store
lacks; `run` of that source does the same check at its end (`--storage-root`
or `STORAGE_ROOT_FOLDER`). `worlds-limits` seeds `worlds.world_limit_overrides` from a copy of
the old worlds service's whitelist document. Both are described in
`sources/worlds-content-server/MAPPING.md`.

Options of `run`:

| Option | Effect |
| --- | --- |
| `--dry-run` | Plans the walk a real run would take from the stored checkpoints, then loads every selected step of every selected source in one target transaction and rolls it back, so later steps and sources see what earlier ones would write. Prints row counts per step and writes nothing. Each step's extract is staged whole on local disk, so `--batch-size` does not apply. |
| `--step NAME` | Only these steps (repeatable). |
| `--since TS` | Re-imports rows whose watermark is at or after `TS` (ISO 8601). Steps without a watermark re-scan fully. |
| `--since last` | `--since` the start of the last completed run of each step, minus `--overlap`. |
| `--overlap DURATION` | Safety margin for `--since last` (default `10m`): covers writes that committed after a later timestamp was already read. |
| `--restart` | Ignores the stored checkpoint and starts the step over. |
| `--batch-size N` | Rows per batch (default 5000). |
| `--skip-unconfigured` | Skips sources whose database variable is unset (with `all`). |
| `--storage-root DIR` | The node's storage root (default `STORAGE_ROOT_FOLDER`), required by runs of steps whose rows name blobs. |

A manifest that does not validate blocks only the runs that select it or run
after it. `check` validates every manifest; with a source's variable set it compares
the extract's output columns with the manifest, and with `TARGET_DATABASE_URL`
set it runs each load against an empty `stage` and rolls back.
`tooling/tests/test_migration.py` does the latter against a fresh rendering of
`schemas/`, so a schema change that breaks a mapping fails the tooling tests.
Its database tests start a throwaway cluster with `initdb`, or use the server
at `MIGRATION_TEST_SERVER_URL` (a disposable one: they drop and create
databases there).

Database errors are printed terse, without the failing row; `--verbose-errors`
(before the command) adds PostgreSQL's DETAIL, which can print row values.

## How a step runs

Each step is a keyset walk over its extract query, ordered by its cursor:
`(watermark, key...)` when the step has a watermark, `(key...)` otherwise.
Each batch:

1. runs `COPY (SELECT * FROM (<extract>) e WHERE <cursor> > <checkpoint> [AND e.<watermark> >= <since>] ORDER BY <cursor> LIMIT <batch>) TO STDOUT` on the source;
2. in one target transaction: creates the temporary table `stage` with the
   manifest's columns, copies the batch into it, runs the step's `load` SQL
   and records the batch's last cursor in `dcl_migration.checkpoint`;
3. commits.

Each batch also takes an advisory lock and checks that the checkpoint is
still the one this run last wrote, so two runs of one step cannot interleave.
A crash loses at most the batch in flight; the next `run` starts after the
last committed cursor. A step whose walk reaches the end is marked complete;
the next `run` without `--since` continues from its cursor and picks up only
rows whose watermark moved past it. A step without a watermark re-scans fully
on every run. Watermarks are compared with the source's clock, and a walk's
start time is read from the source, so `--since last` does not depend on the
machine running the tool.

The engine creates the `dcl_migration` schema (`checkpoint`, `blob_inventory`,
`blob_missing`, `world_limit_seeded`, and the `*_loaded` records of the
notifications-workers and world-storage-service reconcile steps) in the target on the first real run. It is outside the
catalyst schema, which only checks for missing objects; drop it when the
migration is over.

Deletions in a source are not detected: a re-run upserts, it never removes
rows that disappeared upstream. A load may map an upstream soft-delete or
status column. A `whole = true` step (no watermark) stages its full extract
in one batch on every run, never resuming part way, so its load sees the
whole upstream set and can delete what is missing from it; it must only
delete rows it can prove an import wrote (the notifications-workers
reconcile steps keep a record in `dcl_migration`). Its set must fit one
transaction.

## Manifests

`sources/<name>/source.toml`:

```toml
name = "places"                                   # matches the directory
repository = "github.com/decentraland/places"
revision = "<commit the mapping was written against>"
database_env = "PLACES_DATABASE_URL"
after = []                                        # sources migrated first
notes = """What is not migrated and why."""

[[step]]
name = "place"                                    # [a-z0-9_], unique in the source
targets = ["explore.place", "explore.place_moderation"]
extract = "place.extract.sql"                     # one SELECT on the source
load = "place.load.sql"                           # SQL on the target, reads `stage`
key = ["id"]                                      # unique, non-null in the extract
watermark = "updated_at"                          # optional non-null timestamptz
whole = false                                     # optional: true loads the full extract in one batch
columns = [                                       # the extract's output, in order,
  { name = "id", type = "uuid" },                 # typed for the target
  { name = "updated_at", type = "timestamptz" },
]
```

Steps run in manifest order; order them so a load never needs rows a later
step writes. Rules for the SQL:

- **extract**: a single `SELECT`, no `ORDER BY`, `LIMIT` or trailing `;`.
  Output exactly `columns`, by name. Cast upstream enums and domain types to
  `text`; output timestamps as `timestamptz` (an upstream
  `timestamp without time zone` holding UTC becomes `col AT TIME ZONE 'UTC'`).
  Key and watermark columns are never null.
- **load**: runs inside the batch transaction; no `BEGIN`, `COMMIT`, `VACUUM`,
  `TRUNCATE` or `\` commands. psql variables `:'source'`, `:'step'` and
  `:dry_run` are set. Idempotent: `INSERT ... ON CONFLICT ... DO UPDATE`, so
  replaying a batch yields the same rows. Preserves source identifiers. Never
  deletes or truncates target rows unless the manifest notes say why. Sessions
  run with `TimeZone=UTC`.
