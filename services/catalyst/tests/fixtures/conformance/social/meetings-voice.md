# Meetings, screen sharing, streaming and voice chat

What dcl-one needs to hold a meeting: share a screen or a camera into a scene, present slides,
stream from OBS, and talk over voice (nearby, private calls, community voice).

- Upstream: `decentraland/comms-gatekeeper` @ `b42d45c` (main, 2026-11-16);
  `decentraland/social-service-ea` @ `b0cf87e` (main, 2026-11-16; only Node image re-pins since
  `b6b2cef`, the pin in [`PARITY.md`](PARITY.md)).
- dcl-one: `decentraland/dcl-one` @ `f695ad63b` (main, 2026-11-16).
- Method: static reading only. Nothing was built or run for this file. A row names the dcl-one test
  that pins it, or says "untested".
- Paths: `GK/` is upstream comms-gatekeeper `src/`; `C/` is dcl-one `services/catalyst/`;
  `G/` is `C/comms/src/gatekeeper/`; `B/` is `rendering/bevy-explorer/source/`.
- Admission and token claims are in [`gatekeeper-auth.md`](gatekeeper-auth.md). Per-route social
  voice verdicts are in [`PARITY.md`](PARITY.md) and [`RPC.md`](RPC.md).

## Summary

| Capability | dcl-one today |
|---|---|
| Nearby (proximity) voice in scenes and islands | Server and Bevy client present. No multi-client run against a real LiveKit. |
| Private 1:1 calls | Server routes and RPCs at parity; client flow in the Bevy engine and the shared HUD. |
| Community voice (stage, raise hand, promote, kick, end) | Server at parity. Client lacks moderator mute (MV-6, MV-7). |
| OBS/RTMP stream onto a scene screen | Served: `/scene-stream-access` and LiveKit Ingress into the scene room. Not run end to end. |
| Share a screen or camera from a browser (Decentraland Cast) | Not available: every `/cast/*` route answers 501, and no dcl-one client publishes video. |
| Presentations (slides) and presenters | Not available: no presentation-bot token, no presenters routes, no presenter service. |

## How upstream Cast works

Cast publishes into the scene's own LiveKit room, the one `/get-scene-adapter` hands out
(`GK/logic/cast/cast.ts:192-194`).

1. A scene admin calls `GET /cast/generate-stream-link`, signed as the scene, from the Admin Toolkit.
   It checks platform bans and owner/admin or world streaming permission
   (`GK/adapters/scene-manager.ts:39-59`), creates or reuses an RTMP Ingress valid 4 days, and returns
   `{CAST2_URL}/s/{streamingKey}` and `{CAST2_URL}/w/{location}` (`cast.ts:27-37,104-174`).
2. The streamer's browser (cast2 web app) calls `GET /cast/stream-info/:streamingKey`, then
   `POST /cast/streamer-token`: identity `stream:{placeId}:{uuid}`, allowed every track source, added
   to the room metadata `presenters` (`cast.ts:267-288`). It publishes camera, screen and audio.
3. Watchers outside the world call `POST /cast/watcher-token` (signed by a user wallet; platform and
   scene bans checked): a subscribe-only `watch:` token (`cast.ts:368-443`).
4. `POST /cast/presentation-bot-token` gives `cast-presenter-service` a token that publishes slides as
   a screen-share track (`cast.ts:455-500`).
5. `GET/PUT/DELETE /cast/presenters[/:id]`, scene admin only, edit the `presenters` list; scene admins
   are added on join (`GK/controllers/handlers/comms-scene-handler.ts:154-170`). Ordinary scene tokens
   publish only the microphone (`GK/adapters/livekit.ts:135`).
6. In world, a `VideoPlayer` with source `livekit-video://current-stream` shows the active track.

## Cast and streaming routes

| Route | Upstream | dcl-one | Status | Test |
|---|---|---|---|---|
| `POST /cast/streamer-token` | `routes.ts:241`; `cast.ts:242-304` | 501 (`G/mod.rs:871` → `G/streaming.rs:104-106`) | GAP MV-1 | untested |
| `GET /cast/generate-stream-link` | `routes.ts:252`; `cast.ts:185-233` | 501 | GAP MV-1 | untested |
| `GET /cast/stream-info/:streamingKey` | `routes.ts:253`; `get-stream-info-handler.ts:30-93` | 501 | GAP MV-1 | capture `cast-stream-info-invalid-key` (not replayed) |
| `POST /cast/watcher-token` | `routes.ts:246`; `cast.ts:368-443` | 501 | GAP MV-2 | untested |
| `GET/PUT/DELETE /cast/presenters[/:id]` | `routes.ts:291-293`; `cast.ts:512-614` | 501 | GAP MV-4 | `C/comms/src/gatekeeper/streaming.rs` asserts the GET 501 |
| `POST /cast/presentation-bot-token` | `routes.ts:286`; `cast.ts:455-500` | 501 | GAP MV-5 | untested |
| `GET /scene-stream-access` | `list-scene-stream-access-handler.ts:62-80` | `G/streaming.rs:185-192` | MATCH, except no 404 on expiry and `ends_at` forced to `created_at + 4d` (MV-9) | `C/tests/gatekeeper_policy.rs` (PostgreSQL) |
| `POST /scene-stream-access` | `add-…:80-130` | `G/streaming.rs:193-216,295-371` | MATCH | `C/tests/gatekeeper_policy.rs`; `canonical_rooms_tests.rs` |
| `PUT /scene-stream-access` | `reset-…:86-150` | `G/streaming.rs:373-398` | MATCH, except 409/404 wrapped as 500 (MV-9) | `C/tests/gatekeeper_policy.rs` |
| `DELETE /scene-stream-access` | `remove-…:42-65` | `G/streaming.rs:400-413` | MATCH | `C/tests/gatekeeper_policy.rs` |
| Platform bans on stream access (0a6b095) | `add-…:51-59` | `G/streaming.rs:131-136,225-247` | MATCH | `C/comms/src/gatekeeper/streaming.rs` tests |
| Ingress webhooks, 4 h TTL, stale-flag reconcile (cdd006c), key expiry deadline (1e7d0f2), notifications | `adapters/streaming-ttl-checker.ts`, `streaming-key-ttl-checker.ts` | `G/streaming.rs:422-446`; `G/streaming_jobs.rs:28-60,239,278-326` (leased jobs, durable cleanup queue); `G/streaming_notifications.rs:12-60` | MATCH | `G/streaming_jobs.rs` tests |

No ADR records Cast as out of scope. The 501 is stated in `G/streaming.rs:14-15,50-51` and
`C/docs/comms/comms-crate.md:33` only.

## Voice

Server side, every gatekeeper voice route and every social voice HTTP route and RPC is present and
follows the voice-related upstream commits: 4840d22 (stale `participant_left` after rejoin,
`G/voice/lifecycle.rs:312-357`), 0a6b095 (platform bans on voice, `G/voice/private.rs:155-159`,
`G/voice/community.rs:92-97`), defb3ad (bulk cap 100), 50ae662 (`nbf`), 990e4a5 (client URL).
Room names `voice-chat-private-<uuid>` and `voice-chat-community-<uuid>` (`C/comms/src/rooms.rs:138-160`)
match `GK/adapters/livekit.ts:28-29`.

| Area | Upstream | dcl-one | Status | Test |
|---|---|---|---|---|
| `POST /livekit-webhook` participant and room events | `routes.ts:179` | `G/webhook.rs:332-383,528-578`; disconnect reasons `G/voice/lifecycle.rs:172-195,327-394` | MATCH (non-2xx on failure so LiveKit retries) | `G/webhook.rs` ordering tests |
| Private call create / end | `routes.ts:188-193` | `G/voice/private.rs:200,270` | DIVERGENCE: requires a UUID room and two distinct wallets; end answers 200 when the LiveKit room delete fails | `a_failed_room_delete_still_removes_the_call` |
| Community create/join, status, active, bulk status | `routes.ts:196-210` | `G/voice/community.rs:60-242,470` | MATCH | `bulk_community_ids_bounded_at_100` |
| Speak request, promote, demote, mute (metadata) | `routes.ts:212-233` | `G/voice/community.rs:281-410,449` | MATCH | `G/voice/speaker_tests.rs` |
| Kick | `routes.ts:231` | `G/voice/community.rs:412-442` | DIVERGENCE: presence row deleted at once; a failed LiveKit remove still answers success | `C/social/src/rpc/tests/social_api.rs` |
| End community chat | `routes.ts:238` | `G/voice/lifecycle.rs:264-288` | DIVERGENCE: 200 when the room delete fails | `G/tests.rs` |
| Social HTTP (14 routes) and RPC (16 procedures) | `http.routes.ts:283-340`, `rpc.routes.ts:89-183` | `C/social/src/rpc/http.rs:247-285`, `service/private_voice.rs`, `service/community_voice.rs` | MATCH (see [`PARITY.md`](PARITY.md)) | `C/social/src/rpc/tests/` |
| Expiry jobs, TTLs 300 s | `GK/components.ts:203-214` | one leased job every 60 s (`G/mod.rs:658-667`) | MATCH | `C/social/src/rpc/tests/voice_bugs.rs` |
| Nearby voice: microphone-only scene and island tokens, spatial playback | `livekit.ts:135` | `G/admission/handlers.rs:179,247`; `B/crates/comms/src/global_crdt.rs:544-581`, `livekit/mic.rs:463-577` | MATCH (not run against a real LiveKit) | `B/crates/comms/src/livekit/mic.rs` tests |

Client: the Bevy engine signals voice over the social RPC (`B/crates/social/src/voice.rs`) and keeps
the call state machine in `B/crates/comms/src/voice_chat/state.rs`. The shared HUD
(`library/ui/apps/hud/features/voiceChat/VoiceChatPanel.tsx`) covers accepting and refusing calls,
the mic, hanging up, raising a hand, promoting, demoting, kicking and ending a stream.

## Client readiness for video

- The Bevy explorer plays streams: it classifies remote tracks by identity (`stream:`,
  `presentation-bot:`, ingress, screen share; `B/crates/comms/src/livekit/track/plugin.rs:156-193`)
  and shows the priority track on `livekit-video://current-stream`
  (`B/crates/livestream_manager/src/plugin.rs:345-400`, `B/crates/av/src/video_player.rs:151`).
- It publishes only the microphone (`B/crates/comms/src/livekit/mic.rs:706`); scene tokens allow only
  the microphone, as upstream. Screen sharing is meant to go through the Cast web app.
- `apps/sites` `/landings/cast-stream` simulates the token and the screen share
  (`apps/sites/src/features/social/cast-stream/machine.ts:140-175`); `sites` has no `livekit-client`.
  `/landings/cast-watch` calls `GET /cast/stream-info/{location}` and `GET /cast/watcher-token/{location}`
  (`apps/sites/src/data/lib/catalyst/landings/cast-watcher.server.ts:24-41`), which is not the upstream
  contract (stream-info takes a key; watcher-token is a signed POST).
- The Creator Hub Admin Toolkit Cast UI points at upstream production
  (`apps/creator-hub/packages/asset-packs/src/admin-toolkit-ui/VideoControl/api.ts:11-16`).

## Backlog

Ordered by what a meeting with screen sharing needs. Sizes: S small, M medium, L large.

1. **MV-1: Cast core in the gatekeeper.** P0, M.
   `generate-stream-link`, `stream-info/:key` and `streamer-token` (a `stream:` identity allowed every
   source, added to `presenters`). The stream access table, ingress creation, admin decision and ban
   gate already exist in `G/streaming.rs`.
2. **MV-2: `POST /cast/watcher-token`.** P0, S. User-signed only, platform and scene bans, subscribe-only.
3. **MV-3: A browser streamer and watcher.** P0, L. Wire `apps/sites` cast-stream and cast-watch to
   `livekit-client` (screen, camera, publish; subscribe and render), and use the upstream contract.
4. **MV-4: Presenters.** P1, S–M. `GET/PUT/DELETE /cast/presenters`, editing the metadata list without
   clobbering it, and scene admins added as presenters on join.
5. **MV-5: Presentation bot.** P1 for slides, M (L with the presenter service).
   `POST /cast/presentation-bot-token` with `canUpdateOwnMetadata`, and host or port
   `cast-presenter-service`.
6. **MV-6: The Bevy client ignores a moderator mute.** P1, S. `sync_local`
   (`B/crates/comms/src/voice_chat/state.rs:738-754`) reads role and speaker but not `muted`; a speaker
   muted by a moderator keeps publishing. Upstream mute is metadata-only, so the client must enforce it.
7. **MV-7: No moderator "mute speaker" action in the HUD and bridge.** P2, S. Add it to
   `VoiceChatAction` and the participant menu.
8. **MV-8: End-to-end run.** P1, M. Two clients, a real LiveKit with Ingress and the webhook: private
   call, community stage, nearby audio, an RTMP stream on a scene screen, and a Cast screen share once
   MV-1 to MV-3 land.
9. **MV-9: `/scene-stream-access` details.** P2, S. GET answers 404 on expiry and uses
   `expiration_time`; PUT maps 409/404 instead of 500.
10. **MV-10: Kick and end report success when LiveKit fails.** P2, S. Return an error or retry the
    removal; today the user may stay connected.
11. **MV-11: Admin Toolkit base URLs.** P1, S. Make the gatekeeper and presenter bases configurable.
12. **MV-12: HTTP-level tests for the bearer voice routes.** P2, M. Only the 401 shapes are captured.
