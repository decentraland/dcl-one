# Comms gatekeeper: authentication, LiveKit tokens and scene/world admins

Conformance backlog for how a client is admitted to comms and receives a LiveKit token, and for
scene and world administration and moderation.

- Upstream: `decentraland/comms-gatekeeper` @ `b42d45c` (main, 2026-11-16).
  `decentraland/worlds-content-server` @ `ebbf8d4` for world permissions.
- dcl-one: `decentraland/dcl-one` @ `f695ad63b` (main, 2026-11-16), Rust workspace `services/catalyst`.
- Method: static reading only. Nothing was built or run for this file, so no verdict here is verified
  by execution. A row names the dcl-one test that pins it, or says "untested".
- Paths: `GK/` is upstream comms-gatekeeper `src/`; `C/` is dcl-one `services/catalyst/`;
  `G/` is `C/comms/src/gatekeeper/`.
- The endpoint list and captures stay in [`ENDPOINTS.md`](ENDPOINTS.md) (comms-gatekeeper section).
  Calls, voice and screen sharing are in [`meetings-voice.md`](meetings-voice.md).

## Scope: what "Auth API" means here

| Name | Upstream | dcl-one |
|---|---|---|
| `POST /auth/challenge`, `POST /auth/livekit-token` | Not an upstream route. No commit in comms-gatekeeper, social-service-ea or worlds-content-server mentions either path. | dcl-one's own former island-token routes, retired: `C/comms/src/archipelago/mod.rs:691` (`RETIRED_POSTS`, pinned by `the_retired_routes_are_gone`), listed as dropped in `C/docs/comms/comms-crate.md:45`. The repo-root gateway still publishes both (`infra/gateway/routes.nix:43-50`); they reach the 404 fallback. |
| Challenge + LiveKit token | `POST /get-scene-adapter`, `POST /get-server-scene-adapter` (`GK/controllers/routes.ts:131-132`) | `G/admission/handlers.rs` |
| Admin auth (scenes and worlds) | `/scene-admin`, `/scene-bans`, platform moderation (`routes.ts`) | `G/moderation/` |
| `/admin/auth/*` | None | dcl-one's operator console sign-in (`C/src/ops/admin/mod.rs:191-194`): SIWE nonce HMAC-keyed by `SESSION_SECRET`, EOA signature, `ADMIN_ADDRESSES` allow-list, `cat_admin` cookie (HttpOnly, SameSite=Strict, 12 h default). Not a parity surface. |

## 1. Adapter routes (`/get-scene-adapter`, `/get-server-scene-adapter`)

| Behavior | Upstream | dcl-one | Status | Test |
|---|---|---|---|---|
| Signed fetch: crypto-middleware 6.x payload, legacy form only with canonical metadata keys (37776fc, 315f59d) | `GK/logic/utils.ts:23-48` | `G/auth.rs:196-258`, keys `:53-63` | MATCH | `either_shape_with_mixed_case_metadata_verifies_through_the_allowlist` |
| Re-spelled `signer` key refused (637d2a4) | crypto-middleware 6.3.0 | `G/auth.rs:117-119,219-226` | MATCH | `the_signer_allowlist_refuses_missing_respelled_and_unlisted_signers` |
| Signer gate on the adapter routes | none (`utils.ts:34-103`) | `/get-scene-adapter` refuses the scene signer, a re-spelled key too (`verify_wallet_signed_fetch`, `G/auth.rs`); `/get-server-scene-adapter` has no gate | DIVERGENCE (stricter) | `the_scene_adapter_refuses_the_scene_signer` |
| Any verify failure answers 401 | `utils.ts:46-48` | `handlers.rs:106,231` | MATCH | capture `get-scene-adapter-unauth` (not replayed) |
| Realm name source, `/get-scene-adapter` | signed `realmName` only, else 401 (`utils.ts:50-53`) | signed `realmName` only, else 401; `sceneId` signed only, else 400; the body is ignored (`handlers.rs`, `signed_scene_target`) | MATCH | `room_comes_from_signed_realm_name_and_scene_id_only`, `the_scene_adapter_room_comes_from_signed_metadata_not_the_body` |
| Canonical metadata keys | 8 keys | adds `displayName` (`G/auth.rs:48-52`) | DIVERGENCE | untested |
| Platform ban 403, deny list 401, order | platform first (`handlers/comms-scene-handler.ts:46-63`) | deny list first (`handlers.rs:109-111`) | GAP-AU-8 | untested |
| Platform-ban lookup failure | fail open (`access-gate/component.ts:30-36`) | fail closed, 503 (`G/moderation/store.rs:383-400`) | DIVERGENCE (safer) | `undetermined_scene_and_private_message_tokens_request_retry` |
| Genesis scene ban → 403 | `comms-scene-handler.ts:84-136` | `connection_gate` (`store.rs:348-400`) | MATCH | `the_scene_adapter_mints_for_a_clean_wallet_and_refuses_platform_and_scene_bans` |
| World scene ban → 403 "User is banned from this scene" | `comms-scene-handler.ts:116` | enforced inside `can_access`, answers 401 "not authorized to access this world" (`handlers.rs:174-178`) | GAP-AU-7 | untested |
| Genesis `sceneId` is a verified deployment (53deed5) | `GK/adapters/places.ts:200-218` | shared realm takes the id as given (`G/admission/scene_lookup.rs:136-146`) | GAP-AU-4 | format only |
| World scene from a previous deployment maps to the current room by `parcel` (53deed5) | `GK/adapters/worlds.ts:303-345` | retired scenes match, but the room is built from the retired id and `parcel` is never read (`C/worlds/src/rooms.rs:125-143,453-477`) | GAP-AU-2 | untested |
| Legacy `sceneId` = world name, multi-scene world | `parcel` picks the scene (`worlds.ts:311-332`) | newest scene (`C/worlds/src/rooms.rs:300-306`) | GAP-AU-5 | untested |
| World access | owner, unrestricted, allow-list (`worlds.ts:281-291`) | also shared secret and community allow-lists (`G/admission/scene_policy.rs:24-80`) | DIVERGENCE (documented `handlers.rs:85-87`) | `the_world_mints_decide_through_the_permission_core_and_throttle_secrets` |
| Room names (52f32da lowercase) | `livekit.ts:223-245` | `w1:` world scheme, `scene:<realm>:<scene>` (`C/comms/src/rooms.rs:95-134`) | DIVERGENCE (documented `rooms.rs:13-18`, [`worlds-comms.md`](../worlds/worlds-comms.md)) | `canonical_world_client_and_authoritative_tokens_are_byte_identical` |
| Same preview room on both endpoints (101073b), no preview fallback (2de6a61) | server handler `:32-53` | `resolve_adapter_room` (`handlers.rs:118,242`) | MATCH | `server_adapter_uses_signed_context_with_optional_irrelevant_body` |
| Authoritative-server identity → server token | server handler `:40-43,59-60` | `handlers.rs:233-248` | MATCH | `authoritative_server_signature_still_mints_a_token` |
| Scene admins added to room `presenters` on join | `comms-scene-handler.ts:154-170` | absent (Cast not served) | GAP, see [`meetings-voice.md`](meetings-voice.md) MV-4 | – |

## 2. LiveKit token claims

| Claim | Upstream (`GK/adapters/livekit.ts`) | dcl-one | Status | Test |
|---|---|---|---|---|
| Grants: roomJoin, room, canPublish, canSubscribe, canPublishData, canUpdateOwnMetadata, sources `[microphone]` | `:136-145` | `library/dcl-livekit-token/src/lib.rs:84-95` | MATCH | `private_voice_grants_match_upstream_generate_credentials` |
| TTL 300 s, `nbf` at issuance (50ae662) | `:32,132` | `lib.rs:126,156-168` | MATCH | `jwt_has_three_parts_and_omits_absent_claims` |
| Separate client and API endpoints (990e4a5) | `:99-115` | `C/shared/src/livekit.rs:67-141` | MATCH | `endpoints_derive_from_one_host_and_are_overridable_on_their_own` |
| `name`, `metadata` on scene tokens | none | `name`=`displayName`, `metadata={"isGuest":…}`, world authorization attribute, server-token wallet metadata (`handlers.rs:181-205,249-252`) | DIVERGENCE, undocumented (GAP-AU-9) | partial |
| `withNotBefore` keeps the lifetime and rejects a bad `nbf` (841d099) | `:46-60` | replaces `nbf` only (`lib.rs:224-245`), used for island tokens | GAP-AU-6 | `with_not_before_moves_only_the_nbf` pins the old behavior |
| Island grant `canUpdateOwnMetadata` | true (`cluster-subscriber/component.ts:389`) | false, plus owner metadata for the occupant fence (`G/admission/cluster/gateway/island.rs:106-130`) | DIVERGENCE, undocumented; client impact unverified | – |

## 3. Bans at token issue

| Behavior | Upstream | dcl-one | Status | Test |
|---|---|---|---|---|
| Device bans; connection IP/device recorded; null upsert keeps the stored value (21f055b, 0a7c499) | `player-connection-db.ts` | `store.rs:30-37,630-651`; IP from socket or trusted proxy only (`G/auth.rs:455-461`) | MATCH (IP source is a documented divergence) | `a_lookup_without_a_device_id_is_widened_to_the_recorded_device`, `the_scene_adapter_records_the_pinned_client_ip_not_a_client_set_header` |
| Platform bans held in memory (8be80d5) | `adapters/ban-registry/*` | SQL on every check; Pulse polls `GET /bans` with ETag | DIVERGENCE (never stale) | – |
| Island mints check bans **and** the deny list | `cluster-subscriber/component.ts:372-381` | bans and blocked wallets only (`G/admission/cluster/gateway.rs:68-81`); the comment at `subscriber.rs:576-577` says no deny-list store exists, but `CommsState.deny_list` does | GAP-AU-1 | `a_banned_wallet_is_skipped_entirely` (bans only) |

## 4. Scene admins (`/scene-admin`)

| Behavior | Upstream | dcl-one | Status | Test |
|---|---|---|---|---|
| GET: scene or authoritative-server signer | `routes.ts:101-106,140` | `G/moderation/handlers.rs:109-125` | MATCH | `scene_admin_routes_write_the_relations_under_the_scene_signer_gate` |
| GET place, scene signer, bound to the current deployment (53deed5) | `list-scene-admins-handler.ts:39-47` | `G/moderation/context.rs:47-200` (stricter: an old deployment answers 400) | MATCH | `toolkit_signed_scope_rejects_malformed_cross_scene_and_unsigned_redirects_before_authority` |
| GET place, server signer | realm + parcel | query `place_id`, or world name / raw sceneId (`handlers.rs:141-145`), which do not match how rows are keyed | GAP-AU-12 | untested |
| GET response with names, `canBeRemoved`, extras and lease holders; no 500 without profiles (79bdef1) | `list-…:69-100` | `handlers.rs:183-209`; `scene_policy.rs:276-305` | MATCH | untested (capture `scene-admin-signed-scene` not replayed) |
| World extras: owner plus deployment/streaming grantees | `adapters/scene-admins.ts:32-62` | `C/worlds/src/policy.rs:1499-1545` | MATCH | `toolkit_ban_listings_admit_extended_holders` |
| POST/DELETE gate `isSceneOwnerOrAdmin`: owner, admin, extended permissions (LAND operators, world deploy/stream grantees), land lease | `GK/adapters/scene-manager.ts:81-94` | `Policy::is_admin`: explicit admin or owner only (`C/worlds/src/policy.rs:1409-1436`, via `scene_policy.rs:315-361`) | GAP-AU-10 | owner path: `a_land_owner_moderates_a_genesis_scene_without_an_admin_row`; extended writers untested |
| Target protection; add by `name` | `add-…:62-91` | `handlers.rs:59-85,239-258` | MATCH (messages differ) | `scene_ban_rejects_admin_target_and_admin_decisions_are_explicit` |
| Room metadata `sceneAdmins` (1c3b907) | `room-metadata-sync/component.ts:33-161` | `G/admission/scene_effects.rs:206-212`, `maintenance.rs:208-245` (also the world lobby) | MATCH | `canonical_moderation_metadata_and_kick_target_the_joined_scene_and_lobby` |
| Storage key | Places `place.id`, stable across redeploys | `social.scene_admins.place_id` = deployment entity id (`C/comms/src/world_identity.rs:195-201`) | GAP-AU-11 | untested |

## 5. Scene bans (`/scene-bans`, `/scene-bans/addresses`)

| Behavior | Upstream | dcl-one | Status | Test |
|---|---|---|---|---|
| Listing gate includes extended holders and lease | `logic/scene-bans/scene-bans.ts:368-377` | `ensure_can_list_bans` (`handlers.rs:366-393`) | MATCH | `toolkit_ban_listings_admit_extended_holders` |
| Pagination | `getPaginationParams`, offset rounded to the page | limit 100, raw offset (`handlers.rs:310-327`) | GAP-AU-15 | untested |
| POST/DELETE gate | `isSceneOwnerOrAdmin` (`scene-bans.ts:165-168`) | owner or explicit admin (`handlers.rs:490-492`) | GAP-AU-10 | untested |
| Persist, then kick and update metadata | `scene-bans.ts:190-216` | transaction with a durable removal job (`store.rs:237-264`) | MATCH (stronger) | `notification_failure_rolls_back_scene_ban_before_kick_or_room_metadata` |
| World ban from a previous deployment (`allowPreviousDeployment`) | `scene-bans.ts:157-162` | needs the current deployment | DIVERGENCE (stricter) | untested |
| Storage key | Places id | worlds `world:base` (stable); Genesis entity id (`C/comms/src/access.rs:66-84`) | GAP-AU-11 (Genesis only) | `world_ban_keys_on_resolved_scene_id_not_world_name` (worlds) |
| Removed-place cleanup skips opted-out places (2f425e4) | `scene-bans.ts:442-487` | deletes by `explore.place` id (`maintenance.rs:73-79`), which matches neither key | GAP-AU-14 (unverified at runtime) | untested |

## 6. Platform moderation

| Behavior | Upstream | dcl-one | Status | Test |
|---|---|---|---|---|
| Moderator: `MODERATOR_TOKEN` or a wallet in the `platform_user_moderators` feature flag | `logic/moderator/component.ts:19-102` | token or static `PLATFORM_USER_MODERATORS` (`G/auth.rs:411-452`) | DIVERGENCE (static list) | `the_moderator_gate_accepts_the_token_or_a_listed_wallet_and_nothing_else` |
| Ban create: 409 duplicate, advisory lock (defb3ad), device snapshot, kick everywhere | `user-moderation/component.ts:60-97` | `handlers.rs:704-747`, `store.rs:472-536` | MATCH | `concurrent_create_ban_yields_single_active_ban` |
| `GET /users/{a}/bans`: device id stripped (70054bf) | `ban-status-handler.ts:23-48` | `store.rs:434-453,687-773` | MATCH | `the_public_status_shape_never_carries_the_device_id_and_the_moderator_list_keeps_it` |
| `GET /users/{a}/bans`: device match answers `matchedOn:"device"` without the record; own ban `matchedOn:"address"` | `ban-status-handler.ts:38-48` | address-only query, no `matchedOn` (`store.rs:434-453`) | GAP-AU-3 | untested |
| `GET /users/{a}/bans` with a malformed address | 200 `{isBanned:false}` (capture `user-bans-bad-address`) | 400 | GAP-AU-13 | untested |
| `GET /bans` | `routes.ts:284` | `handlers.rs:842-892` (+ ETag/304) | MATCH (superset) | `bans_etag_conditional_get_keeps_envelope_and_empty_304` |
| World ban-status (bearer) | fail open, 200 `false` on any error (`world-ban-check-handler.ts:27-52`) | fail closed, 503 (`handlers.rs:581-620`) | DIVERGENCE (safer; recorded in [`worlds-comms.md`](../worlds/worlds-comms.md)) | `world_ban_status_store_failure_is_undetermined` |
| `POST /mute` | 404 "Deprecated endpoint" (`mute-handler.ts:4-6`) | generic 404 | GAP-AU-15 | untested |
| `/scene-participants` | `scene-participants-handler.ts:34-49` | `G/admission/handlers.rs:282-356` | MATCH | `participant_resolution_preserves_raw_lookup_misses_and_route_statuses` |
| Revocation on world permission edits | webhook refresh, 60 s cooldown | `CommsState::revoke` (`G/revocation.rs:19-60`) from `C/worlds/src/core/mod.rs:252-257` | MATCH | `canonical_world_revocation_reaches_a_connected_room_after_redeployment` |

## Upstream commits since the endpoint pin (`105df71..b42d45c`)

| Commit | Topic | dcl-one |
|---|---|---|
| 52f32da | Lowercase LiveKit room names | DIVERGENCE: own `w1:` scheme |
| 2f425e4 | Keep serving opted-out world scenes | MATCH for admission; cleanup key GAP-AU-14 |
| d366aef | Recover missing island assignments from Pulse | DIVERGENCE: own Pulse (`cluster_lookup`) |
| 53deed5 | Bind scene permissions to verified deployments | MATCH for moderation; GAP-AU-2, GAP-AU-4 for admission |
| 1e7d0f2, cdd006c | Stream access expiry, ingress reconcile | MATCH, see [`meetings-voice.md`](meetings-voice.md) |
| 8ea4e62 | Presentation bot metadata, presenter place lookups | Cast not served, see [`meetings-voice.md`](meetings-voice.md) |
| 841d099 | Finish takeovers before issuing credentials | MATCH with a durable store; `withNotBefore` lifetime GAP-AU-6 |
| 90475d6, 33d2e28, edcaebb, 700fe2b, b42d45c | OpenAPI, tests, Node images | N/A |

## Backlog

1. **GAP-AU-1: Island tokens skip the deny list.** Severity P1.
   - Expected: upstream refuses an island mint for a banned or deny-listed wallet.
   - dcl-one: `G/admission/cluster/gateway.rs:68-81` consults bans only, so a deny-listed wallet receives island credentials. Fix the stale comment at `subscriber.rs:576-577` with it.
2. **GAP-AU-2: Joins from a previous world deployment land in another room.** Severity P1.
   - Expected: the signed `parcel` maps an old deployment's scene to the current scene's room.
   - dcl-one: the room is built from the retired id; `parcel` is ignored. Start with a test that pins the current behavior.
3. **GAP-AU-3: Public ban status ignores device bans.** Severity P1.
   - Expected: `{isBanned:true, matchedOn:"device"}` when another wallet's ban covers the recorded device.
   - dcl-one: address-only query; no `matchedOn`.
4. **GAP-AU-4: Genesis `sceneId` is not checked against a deployed entity.** Severity P2. Any string mints `scene:<realm>:<id>`.
5. **GAP-AU-5: Legacy world-name `sceneId` ignores `parcel` in multi-scene worlds.** Severity P2.
6. **GAP-AU-6: `with_not_before` shortens island tokens.** Severity P2. Keep the lifetime and reject a non-positive `nbf`, as upstream does.
7. **GAP-AU-7: A world scene ban answers 401 "not authorized to access this world".** Severity P2. Upstream answers 403 "User is banned from this scene".
8. **GAP-AU-8: Check order.** Severity P2. A wallet both deny-listed and platform-banned gets 401; upstream gives 403.
9. **GAP-AU-9: Undocumented token claims and realm sources.** Severity P2. Record `name`, `isGuest` metadata, server-token metadata, the island `canUpdateOwnMetadata=false`, `displayName` as a canonical key as divergences, or remove them. `/get-server-scene-adapter` and streaming still fall back to `realm.serverName` (`realm_name_from_metadata`). The web place-page scene chat signs `/get-scene-adapter` as `dcl:explorer`.
10. **GAP-AU-10: Extended holders cannot write admins or scene bans.** Severity P1.
    - Expected: LAND operators, world deployment and streaming grantees and land-lease holders may add and remove admins and scene bans (`isSceneOwnerOrAdmin`).
    - dcl-one: only an explicit admin or the owner (`Policy::is_admin`). The listing gate already uses the extended check (`holds_extended_permission`, `scene_policy.rs:410`); reuse it for writes.
11. **GAP-AU-11: Scene admins, and Genesis scene bans, are lost on redeploy.** Severity P1.
    - Expected: keyed on the stable Places id.
    - dcl-one: keyed on the deployment entity id. World bans already use a stable `world:base` key; give admins and Genesis bans a stable key too (base parcel), with a schema migration step.
12. **GAP-AU-12: The authoritative server's admin listing reads the wrong key.** Severity P1. Resolve the place from the signed realm and parcel, as the scene signer path does.
13. **GAP-AU-13: `GET /users/{a}/bans` with a malformed address answers 400.** Severity P2. Upstream and the capture answer 200 `{isBanned:false}`. Replay the gatekeeper captures in a test.
14. **GAP-AU-14: Removed-place cleanup deletes by a key no row uses.** Severity P2. Unverified at runtime; write the test first.
15. **GAP-AU-15: Small shape differences.** Severity P2. Ban listing offset rounding, `/mute` body, add/remove admin error texts.
16. **GAP-AU-16: The gateway publishes the retired `/auth/challenge` and `/auth/livekit-token`.** Severity P2. Remove the two rows from `infra/gateway/routes.nix`; nothing reads their probes.

Not verified: the upstream signed-fetch freshness window (crypto-middleware is not installed in the
checkout); whether explorers write their own metadata in island rooms; preview-realm event
suppression (dd34391).
