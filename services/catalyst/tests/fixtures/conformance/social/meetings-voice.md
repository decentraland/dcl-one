# Meetings, screen sharing, streaming and voice chat

What dcl-one needs to hold a meeting: share a screen or a camera into a scene, present slides,
stream from OBS, and talk over voice (nearby, private calls, community voice).

- Upstream: `decentraland/comms-gatekeeper` @ `b42d45c` (main, 2026-11-16);
  `decentraland/social-service-ea` @ `b0cf87e` (main, 2026-11-16; only Node image re-pins since
  `b6b2cef`, the pin in [`PARITY.md`](PARITY.md)).
- dcl-one: `decentraland/dcl-one` @ `f695ad63b` (main, 2026-11-16); the Cast sections @ `05ca635f8`
  (main, 2026-11-17) with `fix/cast-follow-ups`.
- Method: static reading only, except MV-5, whose named tests ran on 2026-11-17 against the pin above. A row names the dcl-one test
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
| OBS/RTMP stream onto a scene screen | Served: `/scene-stream-access` and LiveKit Ingress into the scene room. The local stack runs the ingress (`tooling/dev.py`); an RTMP push to a key created with the request the API makes reaches the room as H264 and Opus. Not run end to end through the API and a client. |
| Share a screen or camera from a browser (Decentraland Cast) | Served: every `/cast/*` route, and the `apps/sites` streamer and watcher pages. Run headless against a local LiveKit; no multi-client run in the explorer. |
| Presentations (slides) and presenters | Presenters routes and the presentation-bot token served; `POST /cast-presenter-service/presentations` presents PDF slides through a `cast-bot` process (MV-5). No PPTX, embedded video or client composition. |

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
| `POST /cast/streamer-token` | `routes.ts:241`; `cast.ts:242-304` | `G/streaming_cast.rs` `streamer_token` | DIVERGENCE: identity names the credential, display name cleaned (below) | `a_streaming_key_admits_a_streamer_who_becomes_a_presenter` |
| `GET /cast/generate-stream-link` | `routes.ts:252`; `cast.ts:185-233` | `G/streaming_cast.rs` `generate_stream_link` over `issue_access` | MATCH | `a_scene_admin_gets_a_stream_link_that_is_reused_until_it_expires` |
| `GET /cast/stream-info/:streamingKey` | `routes.ts:253`; `get-stream-info-handler.ts:30-93` | `G/streaming_cast.rs` `stream_info` | MATCH | `stream_info_answers_the_place_behind_a_live_key`; capture `cast-stream-info-invalid-key` replayed by `the_gatekeeper_answers_its_stateless_captures_as_production` |
| `POST /cast/watcher-token` | `routes.ts:246`; `cast.ts:368-443` | `G/streaming_cast.rs` `watcher_token` | DIVERGENCE: a world's access policy is checked, and the token carries the wallet and the world authorization proof (below) | `a_viewer_watches_a_parcel_or_world_stream_unless_banned`; `a_private_worlds_cast_refuses_watchers_the_world_does_not_admit` |
| `GET/PUT/DELETE /cast/presenters[/:id]` | `routes.ts:291-293`; `cast.ts:512-614` | `G/streaming_cast.rs` `list_presenters`, `promote_presenter`, `demote_presenter` | MATCH | `scene_admins_list_promote_and_demote_presenters`; capture `cast-presenters-unauth` replayed by `the_gatekeeper_answers_its_stateless_captures_as_production` |
| `POST /cast/presentation-bot-token` | `routes.ts:286`; `cast.ts:455-500` | `G/streaming_cast.rs` `presentation_bot_token` | DIVERGENCE: identity names the credential | `a_streaming_key_admits_the_presentation_bot` |
| `GET /scene-stream-access` | `list-scene-stream-access-handler.ts:62-80` | `G/streaming.rs:185-192` | MATCH: the stored expiry is answered, and a credential past it answers 404 `Streaming access has expired` | `C/tests/gatekeeper_policy.rs` (PostgreSQL) |
| `POST /scene-stream-access` | `add-…:80-130` | `G/streaming.rs:193-216,295-371` | MATCH | `C/tests/gatekeeper_policy.rs`; `canonical_rooms_tests.rs` |
| `PUT /scene-stream-access` | `reset-…:86-150` | `G/streaming.rs:373-398` | MATCH for 404 (no active credential); a concurrent change inside the reset answers 500, not compared with upstream (MV-9) | `C/tests/gatekeeper_policy.rs` |
| `DELETE /scene-stream-access` | `remove-…:42-65` | `G/streaming.rs:400-413` | MATCH | `C/tests/gatekeeper_policy.rs` |
| Platform bans on stream access (0a6b095) | `add-…:51-59` | `G/streaming.rs:131-136,225-247` | MATCH | `C/comms/src/gatekeeper/streaming.rs` tests |
| Ingress webhooks, 4 h TTL, stale-flag reconcile (cdd006c), key expiry deadline (1e7d0f2), notifications | `adapters/streaming-ttl-checker.ts`, `streaming-key-ttl-checker.ts` | `G/streaming.rs:422-446`; `G/streaming_jobs.rs:28-60,239,278-326` (leased jobs, durable cleanup queue); `G/streaming_notifications.rs:12-60` | MATCH | `G/streaming_jobs.rs` tests |

Cast differences from upstream (each pinned by a test in `G/cast_tests.rs`):

- A world has one stream credential for all its scenes. `/scene-stream-access` and
  `/cast/generate-stream-link` admit there its owner, an admin of the world place, a deployment or
  streaming grantee without a parcel restriction, and an admin granted through `/scene-admin` on the
  scene the request is signed from (that grant is keyed on the scene's entity id, as upstream keys
  it on the world's place). A grantee of some parcels is refused (403) in a world; on a Genesis
  place it is admitted. Pinned by
  `a_world_published_here_answers_every_scene_route_under_its_stripped_realm_name`
  (`C/tests/gatekeeper_policy.rs`).
- Publisher identities name the credential that minted them (`stream:<place>:<access id>:<uuid>`,
  `presentation-bot:<room>:<access id>:<uuid>`). Revoking or replacing a key removes its WebRTC
  publishers from the room, and the `participant_joined` webhook removes one that joins later with a
  token of a revoked key. Upstream only deletes the RTMP ingress.
- A replacement (expiry renewal or reset) stores the new credential in the transaction that
  deactivates the old one: a failure keeps the old key in force, so a reset that fails leaves the key
  it meant to replace working until the admin retries or revokes. A reset intent whose replacement
  is still missing after an hour is delivered as a revocation.
- `presenters` keeps at most 32 streamer ids of a place, written in one read-modify-write: ids whose
  token expired unused go first, then the oldest not in the room. A streamer that leaves the room
  leaves it, except the old session of a full reconnect, and one that rejoins with a live credential
  is added back. Upstream appends one per streamer token.
- A display name loses controls and Unicode default-ignorable characters and is NFKC-normalized. A
  name that reads as another wallet's address (full or `0x1234…abcd`, through common Cyrillic and
  Greek lookalikes) or as another viewer's `Guest#` default becomes `Guest#<last 4>` (watcher) or
  `Streamer`, and so does a blank one. Lookalikes outside that fold are not caught.
- A watcher token checks a world's access policy (allow-list, password, NFT, bans), carries the
  viewer's wallet (`dcl.castWatcher`) and, in a world, the authorization proof a scene join carries,
  so the ban and world access sweeps reach watchers already in the room.
- Joining scene admins become presenters only in rooms with an active credential, off the join's
  response, at most 8 lookups at once and one per room and wallet a minute.

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
- `apps/sites` serves the streamer at `/cast/s/:token` and the watcher at `/cast/w/:location`
  (`apps/sites/src/features/social/cast-live`), which join the room with `livekit-client` through
  `GET /cast/stream-info/{streamingKey}`, `POST /cast/streamer-token` and the signed
  `POST /cast/watcher-token` (`apps/sites/src/data/lib/catalyst/cast.ts`).
- The Creator Hub Admin Toolkit (Cast, live stream, scene admins and bans) calls the realm's own
  deployment: under a non-Decentraland realm's origin, `<origin>/comms-gatekeeper/...` and
  `<origin>/cast-presenter-service/presentations`; on a `decentraland.org`, `.zone` or `.today`
  realm, upstream's per-service hosts (`serviceUrl` in
  `apps/creator-hub/packages/asset-packs/src/admin-toolkit-ui/fetch-utils.ts`).

## Backlog

Ordered by what a meeting with screen sharing needs. Sizes: S small, M medium, L large.

MV-1 to MV-4 (Cast routes, watcher token, the `apps/sites` streamer and watcher, presenters) and MV-11
(the Admin Toolkit's base URLs) are done; see the Cast table and the client notes above.

5. **MV-5: Presenter service.** P1 for slides, M–L. Upstream: `decentraland/cast-presenter-server` @
   `cae7861`. Served for PDF at `POST <node>/cast-presenter-service/presentations`, the path the Admin
   Toolkit calls (`C/comms/src/cast_presenter/`; the gateway admits a 101 MiB body there). The bot's
   LiveKit connection is a `cast-bot` process per session (`C/cast-bot`, the Explorer's LiveKit SDK
   rev; [ADR](../../../../../../docs/adr/2026-11-17-cast-presenter-bot-process.md)) at `CAST_BOT_BIN`.
   Without it the route answers 503. Matches upstream:
   - Requests: multipart upload or JSON `url` (Google Slides and Drive links rewritten; HTTPS, public
     addresses only, pinned connection, 5 redirects, 100 MiB, 60 s); extension and magic bytes; JSON
     body 1 MiB; `MAX_CONCURRENT_PRESENTATIONS` (default 10, 0 unlimited) answers 429.
   - Answers and statuses: `create-presentation-handler.ts:38-178`. A token LiveKit refuses answers
     401 on the JSON path, before the download, and 500 on the multipart path, as upstream.
   - Data channel: the RFC-4 `Packet.scene` framing on topic `presentation` (`comms.ts:1-86`), with
     commands only from the room metadata's `presenters`. `navigate`, `get-state`, `stop` and
     `overlay:update` are handled; the `presentation:state`, `presentation:stopped` and bot-metadata
     JSON is byte for byte (`component.ts:320-335`, `1405`, `1432-1448`; publisher
     `component.ts:62`, `208-211`).
   - Track and lifetime: one SCREENSHARE VP8 track `presentation`, the last frame repeated every
     500 ms; a session with no remote participant for 5 minutes stops.
   - Tests: `comms::cast_presenter::tests`, and `C/cast-bot/tests/livekit.rs` against a local
     `livekit-server`.
   Divergences:
   - Only the gatekeeper's presentation-bot tokens are taken: the node checks the signature with its
     LiveKit key, the expiry, a `presentation-bot:` identity of the granted room and a grant that can
     publish a screen share, before a slot or a bot. Any other token answers 401 on both paths.
   - Multipart uploads are read at most `MAX_CONCURRENT_PRESENTATIONS` at once (429 past that).
   - PDFs render with hayro, not pdfjs: pixels differ, and annotation appearance streams are not drawn.
   - `ws://` LiveKit URLs are accepted on loopback hosts.
   - The SSRF resolver reads the hosts file.
   Not served:
   - PPTX (422 `Unable to render the presentation`).
   - Embedded video: `video:play` is answered with a `presentation:error` event
     (`video-playback-failed`); the other `video:*` commands are ignored and `videoState` stays
     `idle`.
   - Client composition: no `slide` field, no `/presentations/:id/slides/:file`, `presenter:claim`
     ignored.
   - `/api/drive-video` and `/ping`.
   Not run: a scene client presenting through the Admin Toolkit. The APIs image carries `cast-bot` and
   sets `CAST_BOT_BIN` (`infra/containers/README.md`).
6. **MV-6: The Bevy client ignores a moderator mute.** P1, S. `sync_local`
   (`B/crates/comms/src/voice_chat/state.rs:738-754`) reads role and speaker but not `muted`; a speaker
   muted by a moderator keeps publishing. Upstream mute is metadata-only, so the client must enforce it.
7. **MV-7: No moderator "mute speaker" action in the HUD and bridge.** P2, S. Add it to
   `VoiceChatAction` and the participant menu.
8. **MV-8: End-to-end run.** P1, M. Two clients, a real LiveKit with Ingress and the webhook: private
   call, community stage, nearby audio, an RTMP stream on a scene screen, and a Cast screen share once
   MV-1 to MV-3 land.
9. **MV-9: `/scene-stream-access` details.** P2, S. Compare PUT's answer to a concurrent change (500 here) with upstream's.
10. **MV-10: Kick and end report success when LiveKit fails.** P2, S. Return an error or retry the
    removal; today the user may stay connected.
12. **MV-12: HTTP-level tests for the bearer voice routes.** P2, M. Only the 401 shapes are captured; those are replayed by `the_gatekeeper_answers_its_stateless_captures_as_production`.
