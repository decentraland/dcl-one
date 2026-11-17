# Pipeline boundaries

Resolve these paths relative to the current repository. Search the named symbols after a move. Re-read current ownership instructions before crossing client/backend boundaries.

## Client rooms and retries

Under `rendering/bevy-explorer/source/crates/comms/src/`:

| Decision | Source starting points | Distinctions that affect a proof |
|---|---|---|
| Backoff and health reset | `recovery.rs`: `RetryState`, `failed`, `observe_connection`; `room_connection.rs`: `RoomRecovery`, `observe_stable_connections` | Successful connect versus continuously healthy interval; transient versus terminal failure; server delay versus local cap; paused/failed room versus healthy room. Model finite time separately from Rust floating-point inputs. |
| Room lifecycle | `livekit/room/plugin.rs`: `initiate_room_connection`, `poll_connecting_rooms`, `process_room_events`, `record_disconnect`; `livekit/room/mod.rs`: connection ownership/drop hooks | Queued dial versus owned room; SDK reconnect versus new admission; island removal versus credentialed-room denial; identity conflict scope. Include relevant system registration/order if the claim depends on it. |
| Scene admission and moderation | `room_connection.rs`: `request_adapter`, `finish_credentials`; `moderation.rs`: `SceneAdmission`; sibling crate `ban_block/src/lib.rs`: `BanBlock` | Explicit gatekeeper ban evidence versus generic denial or transport failure; scene/world/platform scope; current owner versus a stale credential completion; connected versus paused moderation lists. Social blocks remain a separate observation. |
| Island handoff | `archipelago.rs`: `settle_island`, `reconnect_websocket`; `websocket_room.rs`: `reconnect_websocket` | Current versus pending assignment; retry state retained during promotion; transport authentication versus stable health; old owner teardown versus new owner eligibility. |

A retry proof that resets on every successful connect can miss repeated short connect/remove cycles. Include such a trace and an interrupted healthy interval; derive current thresholds from source instead of copying a remembered constant.

## Microphone, voice ownership and acknowledgements

The same client source root contains:

| Decision | Source starting points | Boundary to preserve |
|---|---|---|
| Routing and session ownership | `voice_chat/mod.rs`, `voice_chat/state.rs`; `livekit/mic.rs` | Selected session versus proximity fallback, private-chat exclusion, wallet owner changes, call ownership, trusted `SystemApi` activation versus scene RPCs. A routing predicate alone does not prove authorization. |
| Revoke and drain | `livekit/mic.rs`: `MicrophonePublication::revoke`, `complete_publication`, `reconcile_publications`, `queue_insert` | Cancellation sent, task finished, source stopped, and unpublish finished are different events. Check the barrier both before enqueueing and during deferred insertion. Count other publications so releasing one cannot erase another's obligation. |
| Native source termination | `livekit/mic.rs`: `mute_local_track`, `build_audio_local_track`, async publication cleanup | Muted versus RTC enabled, publish completing after revoke, capture abort request versus awaited termination, stopped capture versus in-flight/buffered audio. |
| Browser acknowledgement | `livekit/web/local_audio_track.rs`: `new`, `stop`; `livekit/web/mod.rs`: `request`, replies; `livekit/web/host.rs`: `StopLocalAudio`; crate-root `livekit_web_bindings.js` and its tests | Inner creation failure versus missing reply, correlated stop success versus transport error, disable-before-stop, registry release and late publish completion. A successful Rust future alone proves no browser hardware behavior. |

When the checkout contains `voice_room.rs`, also inspect `VoiceRouting`, `ActiveVoiceRoom` and `update_voice_room` for on-demand session ownership.

Useful implementation probes already exist alongside these modules. Inspect and run the targeted test that reaches the claimed boundary; integration evidence must still name whether it executes native, browser, fake SDK or real SDK behavior.

## Backend authority and Pulse

Under `services/catalyst/comms/src/`, start with `gatekeeper/admission/relay_authority/{policy,proof}.rs`, `gatekeeper/admission/cluster/{fence,safety}.rs`, `gatekeeper/admission/cluster/subscriber/takeover.rs`, `protocol/room_authority.rs`, `pulse/handshake.rs` and `pulse/application_relay/`.

Follow callers to the current room resolver and NATS subjects. Keep distinct: gatekeeper admission/proof authority, Pulse's process-local assignments and unfinished takeovers, live media policy, cleanup receipts, and acknowledgement of retained takeover. State restart/lost-message/reordered-message assumptions. A proof over an authority response does not prove authorization of its producer or persistence across a Pulse restart. Verify wire and room-name contracts against pinned protocol/source evidence when they enter the claim.
