# Camera reel: importing camera-reel-service

The camera reel routes (`/camera-reel-service`, `camera-reel/src/lib.rs`) answer camera-reel-service (`decentraland/camera-reel-service@3e8bd0b`). This page covers moving that service's photos to a node.

## What the import does

`catalyst camera-reel import` reads the old service's database (its `images` table) and its public bucket, and writes `camera_reel.images` and the node's blob store:

```sh
CAMERA_REEL_SOURCE_DATABASE_URL=... CAMERA_REEL_SOURCE_BUCKET_URL=... \
  catalyst camera-reel import --database-url "$DATABASE_URL" [--batch 500] [--concurrency 8] [--dry-run] [--prune]
```

Both source settings may also be passed as `--source-database-url` and `--bucket-url`; neither value is printed. `STORAGE_ROOT_FOLDER` names the blob store, as for `serve`.

- Each photo keeps its id, owner (lowercased), document, visibility and `created_at`. Its image and thumbnail are fetched from `<bucket>/<key>`, the key ending the row's `url` and `thumbnail_url`, and stored by hash; the image key's `<file>` is kept as the photo's file name. A row with no `thumbnail_url`, or whose thumbnail object is missing, gets the 640x360 thumbnail an upload generates.
- An imported photo is unmoderated (`review_status` `ok`), records no photo event and does not count again toward a community's photos. `MAX_IMAGES_PER_USER` is not applied: the old service enforced it when the photos were taken.
- Re-running is safe and resumes: a photo already imported is not fetched again. While it is unmoderated, its visibility and document follow the source. A photo a moderator flagged or rejected here keeps its state, and a photo uploaded here is never changed.
- A row the local table refuses (an owner that is not the document's `userAddress`, a document that is not the photo document, no `created_at`) or whose image is missing in the bucket is skipped; the run goes on. Objects are fetched with timeouts and retried with backoff.
- `--dry-run` fetches and checks everything and writes nothing.
- `--prune` runs after a complete pass and deletes the imported photos whose id the source no longer has, as an owner's delete does (the bytes stay in the store, withheld). Photos uploaded here are never pruned.

The command prints one JSON object: `read`, `imported`, `updated`, `unchanged`, `kept_moderated`, `thumbnails_generated`, `bytes_fetched`, `bytes_stored`, `skipped` (counts by reason), `skips` (each skipped id with its reason) and, with `--prune`, `pruned`.

## URLs

An imported photo keeps its file name (`original_filename`), so reads answer its `url` and `thumbnailUrl` as the old service did: `<API_URL>/api/images/<id>-<file>` and `<API_URL>/api/images/<id>-thumbnail-<file>`. A source row whose thumbnail key is not `<id>-thumbnail-<file>` for its image's `<file>` is skipped (`thumbnail_key`). `GET /api/images/<key>` redirects to `CAMERA_REEL_BUCKET_URL/<key>` when that is set, and otherwise to `<API_URL>/api/files/<key>`, which serves the imported bytes from this node. Leave `CAMERA_REEL_BUCKET_URL` unset (or point it at `/api/files`) once the old bucket is no longer the source of truth.

## Cutover

1. While the old service is live, run the import repeatedly (`--dry-run` first). Each run fetches only new photos and brings visibility and document changes over.
2. Stop writes on the old service (uploads, deletes, visibility changes).
3. Run the import a last time with `--prune`, so deletes made since the first run are applied, and check that the report's `skips` are understood.
4. Restart `serve` so a pruned photo's bytes are withheld on the content routes as well.
5. Point the old host name at this node through the gateway or DNS (the router is mounted at `/camera-reel-service`).
