# comms-gatekeeper → catalyst

Pinned at `edcaebbd9ff9cfa95d9a88d9b2cb76043eae9786` (`src/migrations/` up to
`1791320003000_version-streaming-state`).

| Upstream | Target | Notes |
| --- | --- | --- |
| `scene_admin` | `social.scene_admins` | One row per `(place_id, lower(admin))`: the active grant, else the newest revoked one. Upstream keeps no revocation time; `removed_at` = grant time for revoked rows. Upstream `id` is dropped (the node derives a v5 id from the key). |
| `scene_bans` | `social.scene_bans` | Upstream `id` dropped; key `(place_id, banned_address)`. |
| `scene_stream_access` | `social.scene_stream_access` | Same `id`. Epoch ms → timestamps; `streaming_checked_at = 0` → NULL; `ingress_id = ''` (Cast 2.0) → NULL; missing `generated_by` → `''`; `streaming` only on active rows. Upstream lease columns are dropped and `cleanup_retry_at` starts NULL. |
| `scene_stream_access` with `ingress_cleanup_pending` | `social.scene_stream_cleanup` | An unfinished ingress deletion becomes a due cleanup intent with no catalogue context; `ingress_cleanup_expired` → `notification_kind = 'streaming_key_expired'`, else no notification. A NotFound ingress counts as deleted. |
| `user_bans` | `social.user_bans` | Same `id`; timestamps read as UTC. |
| `user_warnings` | `social.user_warnings` | Same `id`; upstream `created_at` has no target column. |
| `player_connection_info` | `social.player_connections` | Watermark `updated_at`; a null never erases a known value, a newer node row wins. Holds IP addresses and device ids. |

Scene identity: since `1771434939000` upstream keys scene admins and bans by
the Places place UUID, the same id `explore.place` / `explore.world_scene_place`
carry, so `place_id` is copied verbatim. Rows still keyed by a world name
(that migration kept them when Places listed no scene for the world) are
skipped; count them with
`SELECT count(*) FROM scene_admin WHERE place_id !~* '^[0-9a-f]{8}-([0-9a-f]{4}-){3}[0-9a-f]{12}$'`
(and the same on `scene_bans`).

Addresses are lowercased; rows whose subject wallet is not `0x` + 40 hex are
skipped. Actor columns (`added_by`, `banned_by`, ...) are lowercased only when
address-shaped.

Not migrated: `voice_chat_users`, `community_voice_chat_users` (live LiveKit
membership, rebuilt from webhooks on rejoin). The gatekeeper stores neither
`private_messages_privacy` (it forwards social-service's value to LiveKit
metadata) nor player reports, so `social.user_settings` and
`social.player_reports` come from other sources. Node-owned:
`livekit_removal_intents`, `player_report_evidence`, the island and MLS tables.
