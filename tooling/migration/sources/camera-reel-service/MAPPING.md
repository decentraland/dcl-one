# camera-reel-service -> camera_reel

Pinned at `3e8bd0b099400e7c15b95477c1f003c63042d461` (migrations through
`20260923103007_add-wearable-items-index`).

| Step | Upstream | Target |
| --- | --- | --- |
| `image` | `images` | `camera_reel.images` |

- `id`, `is_public`, `metadata` (the client document) are copied;
  `user_address` is lowercased; `created_at` (`timestamp without time zone`)
  is read as UTC. `place_id` and the wearable index derive from `metadata` on
  the node.
- `image_hash`/`thumbnail_hash`: upstream names bytes by S3 object key (the
  last segment of `url`/`thumbnail_url`, `<id>-<file>` and
  `<id>-thumbnail-<file>`). The blob copy (out of scope) must record each key
  with its content hash in `dcl_migration.blob_inventory` (`source =
  'camera-reel-service'`); the step stops on an unmapped key.
- Photos from before thumbnails existed (`thumbnail_url = ''`) use the image
  hash as the thumbnail.
- `review_status` is `ok` on insert and never overwritten (upstream has no
  moderation); `withheld_blobs` stays the node's.
- `is_public` follows upstream until the owner toggles it on the node: the
  load records the value it wrote in `dcl_migration.camera_reel_image_loaded`,
  and a row that no longer matches it keeps the node's value. Rows imported
  before that record existed follow upstream once more.
- A photo whose `metadata.userAddress` differs from `user_address` even
  ignoring case is skipped (the node's CHECK holds them together) and
  counted in a NOTICE.

Not migrated: `url`, `thumbnail_url` (the node renders URLs from the hash).
No watermark (`is_public` changes in place, no `updated_at`); deletions
upstream are not detected.
