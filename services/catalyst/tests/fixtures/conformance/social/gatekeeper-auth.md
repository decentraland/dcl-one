# Comms gatekeeper: authentication, LiveKit tokens and scene/world admins

Conformance backlog for how a client is admitted to comms and receives a LiveKit token, and for
scene and world administration and moderation.

- Upstream: `decentraland/comms-gatekeeper` @ `b42d45c` (main, 2026-11-16).
  `decentraland/worlds-content-server` @ `ebbf8d4` for world permissions.
- dcl-one: `decentraland/dcl-one` @ `f695ad63b` (main, 2026-11-16), Rust workspace `services/catalyst`.
- Method: static reading only. Nothing was built or run for this file, so no verdict here is verified
  by execution. A row names the dcl-one test that pins it, or says "untested".
  The rows on world scene bans at the adapter, the admin and scene-ban write gate, the public ban
  status and presenters on join were read at `f699077ce` (main, 2026-11-19).
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
| Signed fetch: crypto-middleware 6.x payload, legacy form only with canonical metadata keys (37776fc, 315f59d) | `GK/logic/utils.ts:23-48` | `G/auth.rs:193-256`, keys `:53-63` | MATCH | `either_shape_with_mixed_case_metadata_verifies_through_the_allowlist` |
| Freshness window: 60 s back and ahead on every route, `/get-server-scene-adapter` and the moderator gate included | crypto-middleware 6.3.0 (resolved by the upstream `yarn.lock`) `DEFAULT_EXPIRATION` = 60 s (`libs/crypto-middleware/src/types.ts:17`, `verifyExpiration` in `verify.ts:230-253`), no route overrides it | `SIGNED_FETCH_WINDOW_SECS` (`library/dcl-auth/src/signed_fetch.rs`), aliased as `DEFAULT_EXPIRATION_SECS` (`G/auth.rs:42`), whole seconds | MATCH | `a_present_but_empty_timestamp_expires_and_a_stale_one_is_refused` |
| Malformed chain (link order, unsigned link, unknown type, bad ephemeral payload) | 401 | 400 "Invalid chain format" | DIVERGENCE (status only) | – |
| Absent `x-identity-metadata` | signed over an empty string | signed over `{}` (`G/auth.rs:210`) | DIVERGENCE (both clients always send the header) | – |
| Re-spelled `signer` key refused (637d2a4) | crypto-middleware 6.3.0 | `G/auth.rs:114-117,216-224` | MATCH | `the_signer_allowlist_refuses_missing_respelled_and_unlisted_signers` |
| Signer gate on the adapter routes | none (`utils.ts:34-103`) | `/get-scene-adapter` refuses the scene signer, a re-spelled key too (`verify_wallet_signed_fetch`, `G/auth.rs`); `/get-server-scene-adapter` has no gate | DIVERGENCE (stricter) | `the_scene_adapter_refuses_the_scene_signer` |
| Any verify failure answers 401 | `utils.ts:46-48` | `handlers.rs:106,231` | MATCH | captures `get-scene-adapter-unauth`, `get-server-scene-adapter-unauth`, replayed by `the_gatekeeper_answers_its_stateless_captures_as_production` |
| Realm name source, `/get-scene-adapter` | signed `realmName` only, else 401 (`utils.ts:50-53`) | signed `realmName` only, else 401; `sceneId` signed only, else 400; the body is ignored (`handlers.rs`, `signed_scene_target`) | MATCH | `room_comes_from_signed_realm_name_and_scene_id_only`, `the_scene_adapter_room_comes_from_signed_metadata_not_the_body` |
| Canonical metadata keys | 8 keys | adds `displayName` (`G/auth.rs:48-52`) | DIVERGENCE | untested |
| Platform ban 403, deny list 401, order | platform first (`handlers/comms-scene-handler.ts:46-63`) | deny list first (`handlers.rs:109-111`) | GAP-AU-8 | untested |
| Platform-ban lookup failure | fail open (`access-gate/component.ts:30-36`) | fail closed, 503 (`G/moderation/store.rs:383-400`) | DIVERGENCE (safer) | `undetermined_scene_and_private_message_tokens_request_retry` |
| Genesis scene ban → 403 | `comms-scene-handler.ts:84-136` | `connection_gate` (`store.rs:348-400`) | MATCH | `the_scene_adapter_mints_for_a_clean_wallet_and_refuses_platform_and_scene_bans` |
| World scene ban → 403 "User is banned from this scene" | `comms-scene-handler.ts:116` | the permission core's scene ban (the scene's `world:base` key or a successor's) is answered 403 with that message, apart from the world's 401 (`G/admission/scene_policy.rs` `world_access`) | MATCH | `a_world_scene_ban_at_the_adapter_is_answered_apart_from_the_world_refusal` |
| Genesis `sceneId` is a verified deployment (53deed5) | `GK/adapters/places.ts:200-218` | `/get-scene-adapter` mints a shared-realm token only for a scene deployed and not deleted in this node's content and not on its denylist, else 404; an unreadable catalogue is 503 (`G/admission/scene_lookup.rs` `require_genesis_deployment`); `/get-server-scene-adapter` takes the id as given, as upstream. Local previews reach this handler (the Explorer's preview gatekeeper is aliased to it): with `ALLOW_LOCAL_PREVIEW=true` a `b64-` id or a `preview`/`localpreview` realm skips the deployment check, as upstream skips it for preview realms; the ban gates still run. The check precedes the ban gate. The realm half of the room stays free-form (GAP-AU-4) | DIVERGENCE (stricter) | `the_scene_adapter_refuses_a_genesis_scene_that_is_not_deployed`, `the_scene_adapter_answers_503_when_the_genesis_catalogue_is_unreadable`, `the_scene_adapter_refuses_a_denylisted_genesis_scene`, `the_scene_adapter_refuses_a_preview_scene_without_allow_local_preview`, `allow_local_preview_accepts_a_preview_scene_without_a_deployment` |
| World scene from a previous deployment maps to the current room by `parcel` (53deed5) | `GK/adapters/worlds.ts:303-345` | retired scenes match, but the room is built from the retired id and `parcel` is never read (`C/worlds/src/rooms.rs:125-143,453-477`) | GAP-AU-2 | untested |
| Legacy `sceneId` = world name, multi-scene world | `parcel` picks the scene (`worlds.ts:311-332`) | newest scene (`C/worlds/src/rooms.rs:300-306`) | GAP-AU-5 | untested |
| World access | owner, unrestricted, allow-list (`worlds.ts:281-291`) | also shared secret and community allow-lists (`G/admission/scene_policy.rs:24-80`) | DIVERGENCE (documented `handlers.rs:85-87`) | `the_world_mints_decide_through_the_permission_core_and_throttle_secrets` |
| Room names (52f32da lowercase) | `livekit.ts:223-245` | `w1:` world scheme, `scene:<realm>:<scene>` (`C/comms/src/rooms.rs:95-134`) | DIVERGENCE (documented `rooms.rs:13-18`, [`worlds-comms.md`](../worlds/worlds-comms.md)) | `canonical_world_client_and_authoritative_tokens_are_byte_identical` |
| Same preview room on both endpoints (101073b), no preview fallback (2de6a61) | server handler `:32-53` | `resolve_adapter_room` (`handlers.rs:118,242`) | MATCH | `server_adapter_uses_signed_context_with_optional_irrelevant_body` |
| Authoritative-server identity → server token | server handler `:40-43,59-60` | `handlers.rs:233-248` | MATCH | `authoritative_server_signature_still_mints_a_token` |
| Scene admins added to room `presenters` on join | `comms-scene-handler.ts:154-170` | `spawn_admin_presenter_on_join`, only in a room with an active credential (`G/streaming_cast.rs`) | DIVERGENCE (narrower: upstream promotes on every join), see [`meetings-voice.md`](meetings-voice.md) | `an_expired_credential_does_not_promote_admins_on_join` |

## 2. LiveKit token claims

| Claim | Upstream (`GK/adapters/livekit.ts`) | dcl-one | Status | Test |
|---|---|---|---|---|
| Grants: roomJoin, room, canPublish, canSubscribe, canPublishData, canUpdateOwnMetadata, sources `[microphone]` | `:136-145` | `library/dcl-livekit-token/src/lib.rs:84-95` | MATCH | `private_voice_grants_match_upstream_generate_credentials` |
| TTL 300 s, `nbf` at issuance (50ae662) | `:32,132` | `lib.rs:126,156-168` | MATCH | `jwt_has_three_parts_and_omits_absent_claims` |
| Separate client and API endpoints (990e4a5) | `:99-115` | `C/shared/src/livekit.rs:67-141` | MATCH | `endpoints_derive_from_one_host_and_are_overridable_on_their_own` |
| `name`, `metadata` on scene tokens | none | `name`=`displayName`, `metadata={"isGuest":…}`, world authorization attribute, server-token wallet metadata (`handlers.rs:181-205,249-252`) | DIVERGENCE, undocumented (GAP-AU-9) | partial |
| `/private-messages/token` when the wallet's DM privacy cannot be read | minted with `private_messages_privacy: "all"` | 503: peers take the token's value as the wallet's setting for its lifetime | DIVERGENCE (safer) | untested |
| `withNotBefore` keeps the lifetime and rejects a bad `nbf` (841d099) | `:46-60` | replaces `nbf` only (`lib.rs:224-245`), used for island tokens | GAP-AU-6 | `with_not_before_moves_only_the_nbf` pins the old behavior |
| Island grant `canUpdateOwnMetadata` | true (`cluster-subscriber/component.ts:389`) | false, plus owner metadata for the occupant fence (`G/admission/cluster/gateway/island.rs:106-130`) | DIVERGENCE, undocumented; client impact unverified | – |

## 3. Bans at token issue

| Behavior | Upstream | dcl-one | Status | Test |
|---|---|---|---|---|
| Device bans; connection IP/device recorded; null upsert keeps the stored value (21f055b, 0a7c499) | `player-connection-db.ts` | `store.rs:30-37,630-651`; IP from socket or trusted proxy only (`G/auth.rs:471-473`) | MATCH (IP source is a documented divergence) | `a_lookup_without_a_device_id_is_widened_to_the_recorded_device`, `the_scene_adapter_records_the_pinned_client_ip_not_a_client_set_header` |
| Platform bans held in memory (8be80d5) | `adapters/ban-registry/*` | SQL on every check; Pulse polls `GET /bans` with ETag | DIVERGENCE (never stale) | – |
| Island mints check bans **and** the deny list | `cluster-subscriber/component.ts:372-381` | bans and blocked wallets only (`G/admission/cluster/gateway.rs:68-81`); the comment at `subscriber.rs:576-577` says no deny-list store exists, but `CommsState.deny_list` does | GAP-AU-1 | `a_banned_wallet_is_skipped_entirely` (bans only) |
| Island mint when the ban lookup fails | deferred: nothing minted, `dcl_gatekeeper_cluster_access_check_failed_total`, a later hint retries (`cluster-subscriber/component.ts:379-383`) | deferred: nothing minted or remembered, `comms_cluster_access_check_failed_total` only, a later hint or reconnect retries (`G/admission/cluster/subscriber.rs`, `admission_withheld`) | MATCH | `an_access_check_failure_defers_the_assignment` |

## 4. Scene admins (`/scene-admin`)

| Behavior | Upstream | dcl-one | Status | Test |
|---|---|---|---|---|
| GET: scene or authoritative-server signer | `routes.ts:101-106,140` | `G/moderation/handlers.rs:109-125` | MATCH | `scene_admin_routes_write_the_relations_under_the_scene_signer_gate` |
| GET place, scene signer, bound to the current deployment (53deed5) | `list-scene-admins-handler.ts:39-47` | `G/moderation/context.rs:47-200` (stricter: an old deployment answers 400) | MATCH | `toolkit_signed_scope_rejects_malformed_cross_scene_and_unsigned_redirects_before_authority` |
| GET place, server signer | realm + parcel | the signed realm, `sceneId` and `parcel` resolved as for the scene signer, read under the scene's place; a `place_id` must name the signed scene (`G/moderation/handlers.rs` `list_admins`) | MATCH since #1832, recorded here late (stricter: `sceneId` required; the Explorer's authoritative server signs it) | `a_server_signed_admin_list_answers_as_the_resolver_says` |
| GET response with names, `canBeRemoved`, extras and lease holders; no 500 without profiles (79bdef1) | `list-…:69-100` | `handlers.rs` (`list_admins`); `scene_policy.rs` (`get_extra_addresses`) | MATCH, except a Genesis place while the land-authz index is not published: 503 naming `land-authz-index`, where upstream lists best-effort extras (an empty extra set would mark LAND holders removable) | `toolkit_moderation::genesis_admin_listing_and_removal_name_the_unready_land_index` |
| World extras: owner plus deployment/streaming grantees | `adapters/scene-admins.ts:32-62` | `C/worlds/src/policy.rs:1499-1545` | MATCH | `toolkit_ban_listings_admit_extended_holders` |
| POST/DELETE gate `isSceneOwnerOrAdmin`: owner, admin, extended permissions (LAND operators, world deploy/stream grantees), land lease | `GK/adapters/scene-manager.ts:81-94` | `scene_manager_decision`: owner or explicit admin, else an extended holder (`G/admission/scene_policy.rs`); unreadable grants answer 503. A world scene's stream key decides its admins under the granter rule in [`meetings-voice.md`](meetings-voice.md) | MATCH | `a_land_owner_moderates_a_genesis_scene_without_an_admin_row`, `toolkit_ban_listings_admit_extended_holders` |
| Target protection; add by `name` | `add-…:62-91` | `handlers.rs:59-85,239-258` | MATCH (messages differ) | `scene_ban_rejects_admin_target_and_admin_decisions_are_explicit` |
| Room metadata `sceneAdmins` (1c3b907) | `room-metadata-sync/component.ts:33-161` | `G/admission/scene_effects.rs:206-212`, `maintenance.rs:208-245` (also the world lobby) | MATCH | `canonical_moderation_metadata_and_kick_target_the_joined_scene_and_lobby` |
| Storage key | Places `place.id`, stable across redeploys | `social.scene_admins.place_id` = the scene's place, stable across redeploys: a world scene's identity, a Genesis scene's base parcel (`C/comms/src/rooms.rs:286`); a world keys each scene apart, see [`meetings-voice.md`](meetings-voice.md) | MATCH | `a_scene_admin_mints_lists_resets_and_removes_the_stream_key`, `a_redeploy_revokes_the_key_of_a_replaced_scene_the_resolver_still_answers` |

## 5. Scene bans (`/scene-bans`, `/scene-bans/addresses`)

| Behavior | Upstream | dcl-one | Status | Test |
|---|---|---|---|---|
| Listing gate includes extended holders and lease | `logic/scene-bans/scene-bans.ts:368-377` | `ensure_can_list_bans` (`handlers.rs:366-393`) | MATCH | `toolkit_ban_listings_admit_extended_holders` |
| Pagination | `getPaginationParams`, offset rounded to the page | limit 100, raw offset (`handlers.rs:310-327`) | GAP-AU-15 | untested |
| POST/DELETE gate | `isSceneOwnerOrAdmin` (`scene-bans.ts:165-168`) | `scene_manager_decision` (`G/moderation/handlers.rs` `ban_user`, `unban_user`) | MATCH | `toolkit_ban_listings_admit_extended_holders` |
| Persist, then kick and update metadata | `scene-bans.ts:190-216` | transaction with a durable removal job (`store.rs:237-264`) | MATCH (stronger) | `notification_failure_rolls_back_scene_ban_before_kick_or_room_metadata` |
| World ban from a previous deployment (`allowPreviousDeployment`) | `scene-bans.ts:157-162` | needs the current deployment | DIVERGENCE (stricter) | untested |
| Storage key | Places id, stable across redeploys | stable across redeploys: a world scene's `world:base`, a Genesis scene's place, the base parcel its admins use (`SceneBanKey`, `ResolvedRoom::ban_key` in `C/comms/src/rooms.rs`); `/get-scene-adapter`, the relay, Cast watchers, room metadata and the scene-state socket and its sweep (`C/worlds/src/scene_state/ws.rs` `admit`) read that key; a Genesis room no deployment stands behind is refused | MATCH | `world_ban_keys_on_resolved_scene_id_not_world_name`, `a_genesis_redeploy_keeps_the_bans_and_admins_until_the_place_is_removed`, `a_genesis_scene_refuses_a_wallet_banned_on_its_place` |
| Removed-place cleanup skips opted-out places (2f425e4) | `scene-bans.ts:442-487` | deletes the bans made before the removal on the keys of the world's scenes, or on the Genesis base parcel when no enabled place has that base (`G/moderation/maintenance.rs` `cleanup`). An opted-out place is not a removal: Explore's trigger records none for `opt_out` (`schemas/10_explore.sql`) | MATCH | `opt_out_is_not_removal_and_delete_preserves_owner_context`, `a_genesis_redeploy_keeps_the_bans_and_admins_until_the_place_is_removed`, `a_removed_local_world_place_revokes_the_admins_of_its_scenes` |

## 6. Platform moderation

| Behavior | Upstream | dcl-one | Status | Test |
|---|---|---|---|---|
| Moderator signed fetch: current payload only, scene signer refused | `signedFetch` without canonical keys (`routes.ts:255-262`) | `authorize_moderator` (`G/auth.rs:423-464`, the current-payload call `:447-458`); it also guards the report list and evidence download (`G/reports.rs:897,998`), which have no upstream route | MATCH | `the_moderator_gate_accepts_the_token_or_a_listed_wallet_and_nothing_else` |
| Moderator: `MODERATOR_TOKEN` or a wallet in the `platform_user_moderators` feature flag | `logic/moderator/component.ts:19-102` | token or a wallet in the `platform_user_moderators` flag of the refreshed dapps document, read per request (`CommsState::platform_moderators`, `G/mod.rs`); a disabled flag or variant lists none, as upstream. Upstream grants no wallet before a flags document exists; dcl-one admits `PLATFORM_USER_MODERATORS` while none is held: `FF_URL` unset, or never fetched and none stored. A failed refresh keeps the last document with no age limit (`social_dapps_flags_last_refresh_timestamp_seconds`). Refreshed every `FEATURE_FLAG_REFRESH_INTERVAL_IN_MS` (240 s default) | DIVERGENCE (env fallback before first flags document) | `the_moderator_gate_accepts_the_token_or_a_listed_wallet_and_nothing_else`, `a_revoked_platform_moderator_is_refused_without_a_restart`, `dapps_flags_refresh_preserves_the_audience_and_last_good_document` |
| Ban create: 409 duplicate, advisory lock (defb3ad), device snapshot, kick everywhere | `user-moderation/component.ts:60-97` | `handlers.rs:704-747`, `store.rs:472-536` | MATCH | `concurrent_create_ban_yields_single_active_ban` |
| `GET /users/{a}/bans`: device id stripped (70054bf) | `ban-status-handler.ts:23-48` | `store.rs:434-453,687-773` | MATCH | `the_public_status_shape_never_carries_the_device_id_and_the_moderator_list_keeps_it` |
| `GET /users/{a}/bans`: device match answers `matchedOn:"device"` without the record; own ban `matchedOn:"address"` | `ban-status-handler.ts:38-48` | `ban_status` (`G/moderation/store.rs`): the wallet's own ban first, else a ban of its recorded device. The route is public on both sides, so anyone can learn that a wallet shares a device with a banned one; accepted with upstream's shape | MATCH | `a_lookup_without_a_device_id_is_widened_to_the_recorded_device` |
| `GET /users/{a}/bans` with a malformed address | 200 `{isBanned:false}` (capture `user-bans-bad-address`) | 200 `{isBanned:false}` (`G/moderation/handlers.rs` `get_user_ban_status`) | MATCH | capture replayed by `the_gatekeeper_answers_its_stateless_captures_as_production` |
| `GET /bans` | `routes.ts:284` | `handlers.rs:842-892` (+ ETag/304) | MATCH (superset) | `bans_etag_conditional_get_keeps_envelope_and_empty_304` |
| World ban-status (bearer) | fail open, 200 `false` on any error (`world-ban-check-handler.ts:27-52`) | fail closed, 503 (`handlers.rs:581-620`) | DIVERGENCE (safer; recorded in [`worlds-comms.md`](../worlds/worlds-comms.md)) | `world_ban_status_store_failure_is_undetermined` |
| `POST /mute` | 404 "Deprecated endpoint" (`mute-handler.ts:4-6`) | generic 404 | GAP-AU-15 | untested |
| `/scene-participants` | `scene-participants-handler.ts:34-49` | `G/admission/handlers.rs:282-356` | MATCH | `participant_resolution_preserves_raw_lookup_misses_and_route_statuses` |
| Revocation on world permission edits | webhook refresh, 60 s cooldown | `CommsState::revoke` (`G/revocation.rs:19-60`) from `C/worlds/src/core/mod.rs:252-257` | MATCH | `canonical_world_revocation_reaches_a_connected_room_after_redeployment` |

## Upstream commits since the endpoint pin (`105df71..b42d45c`)

| Commit | Topic | dcl-one |
|---|---|---|
| 52f32da | Lowercase LiveKit room names | DIVERGENCE: own `w1:` scheme |
| 2f425e4 | Keep serving opted-out world scenes | MATCH |
| d366aef | Recover missing island assignments from Pulse | DIVERGENCE: own Pulse (`cluster_lookup`) |
| 53deed5 | Bind scene permissions to verified deployments | MATCH for moderation; GAP-AU-2, GAP-AU-4 (realm half) for admission |
| 1e7d0f2, cdd006c | Stream access expiry, ingress reconcile | MATCH, see [`meetings-voice.md`](meetings-voice.md) |
| 8ea4e62 | Presentation bot metadata, presenter place lookups | See [`meetings-voice.md`](meetings-voice.md) |
| 841d099 | Finish takeovers before issuing credentials | MATCH with a durable store; `withNotBefore` lifetime GAP-AU-6 |
| 90475d6, 33d2e28, edcaebb, 700fe2b, b42d45c | OpenAPI, tests, Node images | N/A |

## Backlog

1. **GAP-AU-1: Island tokens skip the deny list.** Severity P1.
   - Expected: upstream refuses an island mint for a banned or deny-listed wallet.
   - dcl-one: `G/admission/cluster/gateway.rs:68-81` consults bans only, so a deny-listed wallet receives island credentials. Fix the stale comment at `subscriber.rs:576-577` with it.
2. **GAP-AU-2: Joins from a previous world deployment land in another room.** Severity P1.
   - Expected: the signed `parcel` maps an old deployment's scene to the current scene's room.
   - dcl-one: the room is built from the retired id; `parcel` is ignored. Start with a test that pins the current behavior.
4. **GAP-AU-4: A Genesis room's realm half is free-form.** Severity P2. The scene id must be a deployed scene, but a shared realm name is not checked, so one valid id opens `scene:<any-realm>:<id>` rooms. Upstream also takes the realm name as signed.
5. **GAP-AU-5: Legacy world-name `sceneId` ignores `parcel` in multi-scene worlds.** Severity P2.
6. **GAP-AU-6: `with_not_before` shortens island tokens.** Severity P2. Keep the lifetime and reject a non-positive `nbf`, as upstream does.
8. **GAP-AU-8: Check order.** Severity P2. A wallet both deny-listed and platform-banned gets 401; upstream gives 403.
9. **GAP-AU-9: Undocumented token claims and realm sources.** Severity P2. Record `name`, `isGuest` metadata, server-token metadata, the island `canUpdateOwnMetadata=false`, `displayName` as a canonical key as divergences, or remove them. `/get-server-scene-adapter` and streaming still fall back to `realm.serverName` (`realm_name_from_metadata`). The web place-page scene chat signs `/get-scene-adapter` as `dcl:explorer`.
15. **GAP-AU-15: Small shape differences.** Severity P2. Ban listing offset rounding, `/mute` body, add/remove admin error texts.
16. **GAP-AU-16: The gateway publishes the retired `/auth/challenge` and `/auth/livekit-token`.** Severity P2. Remove the two rows from `infra/gateway/routes.nix`; nothing reads their probes.

Not verified: whether explorers write their own metadata in island rooms; preview-realm event
suppression (dd34391).
