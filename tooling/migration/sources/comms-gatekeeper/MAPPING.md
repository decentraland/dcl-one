# comms-gatekeeper → catalyst

Pinned at `33d2e28aefad26b72902a8c5d694129df0de3b77` (`src/migrations/` up to
`1791320003000_version-streaming-state`).

| Upstream | Target | Notes |
| --- | --- | --- |
| `scene_admin` | `social.scene_admins` | One row per `(place_id, lower(admin))`: the active grant, else the newest revoked one. Upstream keeps no revocation time; `removed_at` = grant time for revoked rows. Upstream `id` is dropped (the node derives a v5 id from the key). A node grant or revocation later than the upstream grant time is kept on re-runs. |
| `scene_bans` | `social.scene_bans` | Upstream `id` dropped; key `(place_id, banned_address)`. |
| `scene_stream_access` | `social.scene_stream_access` | Same `id`. Epoch ms → timestamps; `streaming_checked_at = 0` → NULL; `ingress_id = ''` (Cast 2.0) → NULL; missing `generated_by` → `''`; `streaming` only on active rows. Upstream lease columns are dropped and `cleanup_retry_at` starts NULL. A credential the node retired stays retired, and node streaming state with a higher `streaming_state_version` is kept. An upstream active credential retires the place's older active one with a cleanup intent (no notice); one older than the node's active credential lands inactive with the same intent. |
| `scene_stream_access` with `ingress_cleanup_pending` | `social.scene_stream_cleanup` | An unfinished ingress deletion becomes a due cleanup intent with no catalogue context; `ingress_cleanup_expired` → `notification_kind = 'streaming_key_expired'`, else no notification. A NotFound ingress counts as deleted. |
| `user_bans` | `social.user_bans` | Same `id`; timestamps read as UTC. A lift recorded on the node (`lifted_at`, `lifted_by`) is never cleared by a re-run; loads take the node's per-address ban lock. |
| `user_warnings` | `social.user_warnings` | Same `id`; upstream `created_at` has no target column. |
| `player_connection_info` | `social.player_connections` | Watermark `updated_at`; a null never erases a known value, a newer node row wins. Holds IP addresses and device ids. |

Scene identity: import Places and worlds first. Upstream scene admins and bans
use Places UUIDs, but the target checks scene admins by entity ID, Genesis bans
by entity ID, and world bans by `world_name:effective_base`. Resolve Genesis
UUIDs through `explore.place.raw.deployment_id` to a live content deployment
with active pointers. Resolve world UUIDs through the active
`explore.world_scene_place` association and its `worlds.world_scenes` row.
World ban bases use the target resolver's rules: the canonical declared base
when it belongs to the footprint, otherwise the numerically first valid parcel.

Both moderation steps read the complete source set in one transaction, so
aliases of one target identity cannot land in separate batches and overwrite
each other. Active/newest admin grants and earliest bans win deterministically.
Size the cutover transaction for the complete moderation set.

Missing or ambiguous bindings, including old world-name keys, abort the batch
before writing moderation rows. Fix those bindings before cutover; never infer a
scene by name or overlap. Genesis content deployments must already be populated
on the destination; the Places import alone does not import content. Streaming
credential IDs retain their separate Places UUID contract and are not rewritten.

Addresses are lowercased; rows whose subject wallet is not `0x` + 40 hex are
skipped. Actor columns (`added_by`, `banned_by`, ...) are lowercased only when
address-shaped.

Not migrated: `voice_chat_users`, `community_voice_chat_users` (live LiveKit
membership, rebuilt from webhooks on rejoin). The gatekeeper stores neither
`private_messages_privacy` (it forwards social-service's value to LiveKit
metadata) nor player reports, so `social.user_settings` and
`social.player_reports` come from other sources. Node-owned:
`livekit_removal_intents`, `player_report_evidence`, the island and MLS tables.
