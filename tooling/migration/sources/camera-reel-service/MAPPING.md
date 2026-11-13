# camera-reel-service -> camera_reel

Pinned at `3e8bd0b099400e7c15b95477c1f003c63042d461` (migrations through
`20260923103007_add-wearable-items-index`).

The existing migration engine imports canonical historical photos into
`camera_reel.images`. The target must already support `original_filename`
and `image_content_type` (the additive filename and MIME capabilities), plus
`ranking_published`. This mapping does not install runtime schema changes.

Set `CAMERA_REEL_IMPORT_API_URL` to the exact source API base, which must also
be the target's literal configured Camera API base. A trailing slash is retained:
`<base>/api/images/` then contains the same double slash as the source URL; it is
not normalized. Export a quiescent source database
and its original objects to a flat local folder; point `CAMERA_REEL_DATABASE_URL`
at the exported database. The migration client needs access to the target's
shared `STORAGE_ROOT_FOLDER`. These commands use that existing database and
blob pipeline:

```sh
python3 tooling/migration/migrate.py blobs camera-reel-service --from-dir "$CAMERA_OBJECTS" --object-metadata "$CAMERA_OBJECT_METADATA" --to "$STORAGE_ROOT_FOLDER"
python3 tooling/migration/migrate.py run camera-reel-service --dry-run --storage-root "$STORAGE_ROOT_FOLDER"
python3 tooling/migration/migrate.py run camera-reel-service --storage-root "$STORAGE_ROOT_FOLDER"
```

`--object-metadata` is required for Camera copies. Supply original object metadata
exported alongside the immutable bytes; the source database does not contain it.
The UTF-8 JSONL format has exactly two fields per record:

```json
{"object_key":"12345678-1234-5678-9012-123456789012-photo%2F.png","content_type":"image/jpeg"}
{"object_key":"12345678-1234-5678-9012-123456789012-thumbnail-photo%2F.png","content_type":"image/jpeg"}
```

Keys are literal flat object names, not URL-decoded paths. MIME must be exactly
`image/png` or `image/jpeg`, and the original and thumbnail of each photo must
have the same value. The copier does not infer MIME from bytes: PNG bytes may
correctly retain an exported `image/jpeg` header. Two photos sharing one CID may
have different MIME values because the header belongs to each source object and
photo alias. Missing metadata or unequal pairs fail before copying or changing
target bookkeeping. Unknown fields, duplicate JSON fields or object keys,
nonregular/symlink manifest files, and records larger than 4096 bytes are rejected.

For N photos and M manifest objects, the CLI streams M records into a temporary
SQLite index with a 2 MiB page cache and performs two key lookups per photo. This
index is discarded after the command; it adds no runtime database or service.
The existing migration-owned `blob_inventory` gains a nullable `content_type`
column through additive ensure DDL. Other sources leave that column null.
Camera copies persist the explicit MIME with the verified CID and size. Retrying
can enrich an old unknown MIME only after recopying bytes whose CID and size
match; it never replaces a known MIME. Publication reads this durable inventory,
requires both headers, and includes MIME in strict target-row equality. Existing
target rows with unknown or different MIME require explicit reconciliation.

`run` and `run --dry-run` preflight source rows against a read-only export of that
inventory before any target mutation. No manifest is needed again once the
copy's verified facts are durable. Each publication batch rechecks the MIME
facts as well as the local blobs under the shared GC lock; missing header facts
never invoke the runtime's legacy byte-sniffing fallback.

Preflight scans the source through a disk spool and reports every unsupported
row before copying or loading. Supported URLs have exactly these forms:
`<API base>/api/images/<UUID>-<basename>` and
`<API base>/api/images/<UUID>-thumbnail-<basename>`. No URL decoding occurs.
Different origins, mismatched names, slash/backslash, control characters and
overlong object keys fail preflight. Raw Unicode, spaces, quotes, `?`, `#` and
literal percent spellings such as `%2F` are retained when both paired URLs and
flat object names match exactly. They are filename characters here, not parsed
URL components; `%2F` and `%252F` remain distinct keys. The configured API base
must not include query or fragment delimiters.
Empty thumbnails, null timestamps, nonnormalized owners, owner disagreement
and metadata that cannot decode as the Camera response type also fail.
Unsupported facts require an explicit cutover decision; they are not replaced
with timestamps, empty metadata, original-as-thumbnail, or newly made thumbnails.

The copier reads exact filenames, including their original bytes. It does not
use the generic storage copier's gzip fallback. It streams both objects through
the existing CIDv1/UnixFS hashing and atomic installation code. Hash reduction
retains at most 173 child nodes per tree level. Copy reporting and inventory
writes use bounded batches. Retries re-read source bytes and refuse to change
an existing source-key-to-CID/size/MIME mapping, detecting a changed snapshot. Shared CIDs
keep one stored file. Missing objects make the copy command fail; missing-object
records never authorize importing a Camera row.

Exact source objects must be regular files in the exported directory. The reader
anchors each lookup to an open directory descriptor, rejects symlinks and special
files, and uses nonblocking, no-follow opens plus file-identity checks so a replaced
leaf cannot redirect a checked read. Keep the export immutable throughout the run;
these checks do not make a concurrently edited snapshot consistent. Source parent
directories, the migration client, Python executable and target blob root must be
operator-controlled. The shared target store and its parent directories must not
be writable by untrusted local users; its normal storage operations are not a
filesystem sandbox.

Publication uses at most 100 photos per transaction. After acquiring storage's
shared GC advisory lock, psql's **client-side** `\copy FROM PROGRAM` hashes the
referenced local files again. This needs a client Python executable and filesystem
access, not database superuser privileges or a server-side program. The lock is
held through row insertion and checkpoint commit. Missing or corrupt staged
files abort the batch; recopy them and resume. Source reads finish before the
target transaction begins. Dry runs use the same bounded publication checks,
then roll back each batch without advancing checkpoints.

Photo UUID, normalized owner, timestamp at microsecond precision, privacy,
metadata JSON, canonical basename, explicit original MIME and both original byte
streams are retained.
An exact UUID replay retains the image facts; differing immutable target facts
abort the batch without updates. The importer records the last visibility it
wrote in `dcl_migration.camera_reel_image_loaded`. Later source visibility changes
follow only while the target still matches that recorded value; local visibility
edits remain intact. Existing UUIDs without an import marker require exact facts
and visibility and are never silently adopted or overwritten. Existing moderation
remains untouched, including when source visibility changes. Historical rows explicitly set
`ranking_published=true`, without new photo events, ranking increments or rewards.
A failed batch rolls back its rows and checkpoint together.

This supports canonical rows only. Before cutover, independently inventory
noncanonical per-row URLs, objects with no database row, missing objects,
historical API/bucket origins, and source deletions. Orphan filenames cannot be
represented by a photo's basename; this importer neither fabricates photo rows
nor claims to preserve their direct links. Routing the historical origin, TLS,
HEAD/Range/cache behavior and a consistent final source snapshot remain cutover
prerequisites. This is an offline import, not continuous synchronization.

Seasonal enrollment/progress and outstanding signed MANA liabilities are outside
this mapping. A Camera import does not settle that dependent financial handover
gap and does not issue replacement rewards or convert liabilities to USD.
