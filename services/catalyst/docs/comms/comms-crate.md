# catalyst-comms: as-built boundaries

`services/catalyst/comms` (`catalyst-comms`, layer 8) owns comms and multiplayer.
The ownership boundary is the [root AGENTS.md "Ownership → comms" bullet](../../../../AGENTS.md#ownership);
this file adds modules, interfaces and channel contracts. Decisions, findings and acceptance evidence live in the task report; use the [record workflow](README.md).

For the C# responsibility map and Rust ownership/threading migration instructions, read the
[Pulse to Rust migration guide](pulse-rust-migration.md). Its proposed worker structure is
separate from the current runtime described here.

## Architectural constraints

- One crate, not four, evolved in place from the moved dcl-one modules.
- Client-facing contracts (HTTP paths, room names, client wire protocol) stay compatible; NATS subjects are internal and change with both ends in one commit.
- Services talk over core NATS, at most once; it was kept over gRPC, Zenoh and Postgres NOTIFY because it alone covers per-key events, snapshots, fan-out and request/reply.
- The gatekeeper runs inside the primary `catalyst serve`; only the primary serves `/ws`. Pulse runs as `catalyst pulse`.
- The shared vocabulary lives in comms; ban status has one answer, comms moderation; the MLS delivery service stays in social.
- Friends' online status comes from Pulse's islands snapshot. Room occupancy and capacity stay on LiveKit as an interim; the target is Pulse.
- Pulse retains unfinished takeover work in memory; gatekeeper alone applies LiveKit cleanup, confirms completion and mints under the authoritative token floor.
- Pulse and the native Bevy client use one pinned, patched ENet source in `library/vendor/rusty_enet`. Each keeps its own transport driver; channel 1 explicitly bypasses local sender throttling while retaining sequenced-unreliable semantics. Pulse implements the opt-in 8192 payload/65536 charged queued bytes/256 command native receive boundary, with typed terminal early reason 16 and exact release; shared Explorer defaults/outgoing limits remain. Ordinary/connected event adapters preserve session ownership. No service/process/channel is added. Check current acceptance evidence in the task report before making runtime claims.
- Public Pulse V4 and Archipelago native/negotiated V4 authentication are separate proposed capabilities, deferred pending a separate user verdict. Ordinary signed `/ws` and its configured ownership remain supported. Profile announcements require nonnegative, strictly increasing versions but do not consume the teleport/emote budget; emote IDs follow the baseline without a separate 512-character cap.
- Scene hosting splits into a comms orchestrator and the engine runtime; Hammurabi is deprecated and never a source. See § Scene host.

## Target crate

| Module | Runs in | Owns |
|---|---|---|
| `pulse` | `catalyst pulse` | avatar sync over ENet UDP (default `:9000`) and WebTransport (`:7743`); islands (`cluster`); the dormant [application relay implementation](../../comms/src/pulse/application_relay/README.md); ban snapshots (`bans`, ETag poll of `GET /bans`); remote flags and per-IP admission (`flags`) |
| `archipelago` | primary `serve` | `/ws`; peer, island and hot-scene stats; the `/about` comms block (`CommsCore`) |
| `gatekeeper::admission` | `serve` | scene, server-scene, world (`world_mint`) and island tokens; the Pulse room authority (`relay_authority`) |
| `gatekeeper::moderation`, `removal` | `serve` | bans, warnings, scene admins, durable ban kicks, disabled-place cleanup, room metadata repair |
| `gatekeeper::voice`, `streaming`, `streaming_jobs` | `serve` | private and community voice; stream access, LiveKit Ingress, streaming expiry; `/cast/*` answers 501 |
| `gatekeeper::webhook`, `events` | `serve` | the one LiveKit webhook; a typed nine-field `RoomEvent` to `RoomEventSink` |
| `gatekeeper::bus` | `serve` | `serve`'s one NATS link |
| `presence`, `topology` | every `serve` | `/live-data`, `/wallet/{w}/connected-world`, capacity counts from LiveKit occupancy; social online from Pulse's topology |
| `protocol` | all | Pulse proto, legacy archipelago, `bus`, `room_authority`: the only module the services share |
| `rooms`, `world_identity`, `access`, `limits`, `worlds_envelope`, `writes`, `signed_fetch`, `deny_list` | library | the vocabulary worlds, social and shop use |

Outside: access, room resolution and storage (worlds); privacy, friendships, roles, notification
records and `/mls/*` (social); the scene runtime; LiveKit. Dropped: the relay to Decentraland's
Pulse (a mirror advertises Decentraland's Pulse in `/about`),
player report routes, `POST /auth/challenge`, `/auth/livekit-token` and
`/heartbeat`. Room recovery uses `cluster_change`, `cluster_lookup` and
`takeover_completed`; hints do not own unfinished work.

## Interfaces

| Comms needs | Implemented by | For |
|---|---|---|
| `interfaces::AccessPolicy` | `worlds::policy::Policy` | may this wallet enter this world, scene or parcel |
| `interfaces::SceneIdentity` | `worlds::rooms::SceneResolver` | a world's active scene and the resolved room; room names are `comms::rooms` |
| `interfaces::SocialDirectory` | `social::comms_directory::Directory` | privacy, friendships, community roles; notices inserted on the caller's transaction |
| `interfaces::NameOwnerReader` | shop `MarketplaceState` | name ownership for Admin Toolkit targets |
| `gatekeeper::events::RoomEventSink` | `worlds::core::CoreState` | worlds' join/leave access log |

| Comms offers | Consumers |
|---|---|
| `CommsState::revoke(RoomRevocation)`, the one revocation entry point | worlds permission edits |
| `explore::places::live::Participants` for `CommsState` | explore live counts, the presence archive |
| ban status, presence, voice rooms | worlds policy and publish; social, `/worlds/status`; social call signalling |
| signed `POST /get-server-scene-adapter`, `GET /scene-participants` | scene hosts |

## Placement

World-scene place identity belongs to `explore::places::scene_identity`
. Worlds invokes it in each deployment transaction under the world
row lock. The shared `explore.world_scene_place` row binds a stable UUID to the
current deployment; storage resolution and scene discovery read that binding.
World aggregate discovery, upstream catalogue IDs, Genesis storage scopes and
comms room-name contracts remain unchanged. The periodic catalogue job refreshes
metadata; it does not own identity lifecycle.

`comms = 8` in `docs/module-layers.toml` (above explore, below worlds, social and shop), enforced by
the repository structure checks. Higher crates implement comms' traits, keeping V8 and worlds out of comms builds.

## Services and how they talk

| Link | Channel |
|---|---|
| connector ↔ gatekeeper | calls inside the primary `serve` (`CommsState::announce_session`, `IslandSockets`); `peer.*.connect`, `peer.*.disconnect`, `engine.peer.*.island_changed` are retired |
| Pulse ↔ `serve` | core NATS, § Bus messages |
| Pulse → gatekeeper | `GET /bans` (moderator bearer, ETag); `POST /internal/pulse/room-authority/v1` with an HMAC key |
| gatekeeper → `multiplayer-hosting` | `RoomEvent` on `comms.room_events.v1` (defined, not yet published) |
| `multiplayer-hosting` → gatekeeper / engine | public routes / stdin-stdout JSON lines |

`/ws` limits : `WS_MAX_SOCKETS` (default 3,000, sized by the 10 ms-per-socket
re-announce sweep's 30 s cadence), beyond it 503 with `Retry-After: 5`; positions coalesce
to one per 500 ms; a flooding socket closes with 1008; a 64-frame send queue in which a newer
`island_changed` replaces the queued one and `kicked` is never dropped, full for 10 s closes
with 1013; undecodable frames 1007; restart closes every socket with 1012.

**Deployment.**
- One primary is a release gate. Every API replica sets `FLEET_JOBS_ENABLED=false` (default true). The proxy sends `/ws`, `/parcels`, `/peers`, `/peers/{id}`, `/islands`, `/islands/{id}` (at the root and under `/comms` and `/stats`), `/hot-scenes`, `/stats/health` (root and `/archipelago-ea-stats`) and `/lambdas/explore/realms` to the primary alone; a replica answers 404 for all but the last. Stop the primary before its successor starts.
- Key separation: only the gatekeeper holds the LiveKit secret; the dormant relay implementation requires a room-authority answer and has no production activation; `multiplayer-hosting` will hold the authoritative key; engines run as a separate user with a scrubbed environment.
- Configured legacy ownership uses `ARCHIPELAGO_CONTROL_V4_AUDIENCE`. Pulse accepts that audience or `PULSE_V4_AUDIENCE` for its shared NATS control-position bridge; conflicting values fail startup. Neither setting enables public V4 authentication.
- `DENY_LIST_URL` is required in production; unset, the node warns at start and runs with the list off.
- `PULSE_FLAGS_URL` (HTTPS or numeric-loopback HTTP, polled every 60 s) may set only the four `PULSE_IP_LIMITER_*` values (`ENABLED` false, `MAX_CONCURRENCY` 10, `SCENE_LISTENER_MAX_CONCURRENCY` 2, `WHITELIST` empty by default).

## Bus messages

Types, subjects and caps: `comms::protocol::bus`. Record reviewed channel contracts in the architecture model.

| Message | From → to | Subject | Cap, cadence | Recovery | NATS permission |
|---|---|---|---|---|---|
| Cluster change | Pulse → primary island subscriber (one unqueued subscription) | `peer.{wallet}.cluster_change` | 64 KiB; coalescing outbox | fresh lookup; rotating retained-pending hints | Pulse pub, `serve` sub |
| Cluster lookup | `serve` → Pulse, reply on private `_INBOX.` | `peer.{wallet}.cluster_lookup` | exact session request; complete reply ≤64 KiB; 2 s deadline | active re-announcement and retained pending cleanup | `serve` pub, Pulse sub; both pub/sub `_INBOX.>` |
| Takeover completion | gatekeeper → Pulse | `peer.{wallet}.takeover_completed` | 1–4096 bytes; ID 1–64 bytes | resend identical ID/cutoff until authoritative acknowledgment | `serve` pub, Pulse sub |
| Islands snapshot | Pulse → connector stats, `presence` | `engine.islands` | 1 MiB, 1 s | next snapshot; online expires 5 s after the last valid one | Pulse pub, `serve` sub |
| Discovery beacon | Pulse → `/core-status`, `/about` | `engine.discovery` | 10 s | healthy while < 90 s old | Pulse pub, `serve` sub |
| Control position | connector → Pulse | `archipelago.control.position.v1` | 640 B | next heartbeat | `serve` pub, Pulse sub |
| Room event | webhook → `multiplayer-hosting` | `comms.room_events.v1` | 4 KiB | reconcile via `GET /scene-participants` | `serve` pub, `multiplayer-hosting` sub only |

A publish returns after bounded local SDK acceptance: `Submitted` proves neither broker authorization nor delivery. Require per-process NATS credentials and subject permissions matching the producer/consumer table. Verify deployment enforcement through the private release review.

## Room recovery

The canonical contract is `library/protocol/proto/decentraland/pulse/pulse_clusters.proto`.
Pulse commits each completed assignment pass and its recovery state before producing hints.
A process-lifetime source incarnation and checked assignment version distinguish replacement
and expiry; unchanged passes preserve identity. Live registration and control-owner checks
invalidate stale active lookups between passes. Ready snapshots record potential credential
exposure, so an intermediate assignment blocked by cleanup does not invent another predecessor.

Pulse budgets 24,573 simultaneous distinct transport/control wallets and one equally sized
churn cohort, for 49,146 retained wallet records, with at most 32 pending operations per
wallet. Complete plans are bounded to 64 KiB; overflow never becomes a truncated ready reply.
Unfinished or blocked records never expire. Other records require both 300-pass retention
and the confirmed floor's 15-second clock grace. Exact retained inactive sessions can finish
pending cleanup without a socket. Their last completion makes lookup unavailable, preventing
inactive admission. Further churn can exhaust the finite reserve; it is not a production
throughput measurement or an unlimited availability guarantee.

A completion removes only its matching retained operation. Its positive whole-second cutoff
must exceed that operation's prior floor and remain within the JavaScript safe-integer range;
the aggregate floor takes the maximum. Duplicate or unknown IDs cannot advance it. Gatekeeper
separately bounds arithmetic, waits for the floor plus its one-second clock allowance and
refuses waits of 60 seconds or more. Signing chooses the greater of the Pulse/durable wallet
floor and current Unix time less that allowance. A positively settled Cloud removal must
finish before the cutoff minus the allowance; otherwise only that known-settled operation
may obtain fresh authority and a higher cutoff. Unknown outcomes never gain a retry.

Gatekeeper owns three typed shared PostgreSQL relations: permanent wallet admission
fences/generations/floors, immutable per-RPC cleanup attempts and logical Pulse takeover
receipts. The key spans all Pulse sources and configured/unconfigured ownership in the
shared database. The unresolved dispatch marker commits before RPC; remote calls start only after that
SQL transaction ends. Short transactions lock wallet before assignment/intent rows;
there is no NATS or SFU await while SQL locks are held. Only primary dispatches cleanup.
Every island removal, including connector and moderation enforcement, uses this same
wallet safety with real operation/intent provenance; enforcement never fabricates a Pulse
operation or completion. Lift/delete/claim expiry and room closure do not delete hazards.

The cap of 10,000 counts outstanding logical obligations, not permanent fence rows or
all historical attempts. Unknown timeout, cancellation or disconnect stays admission-closed
across gatekeeper restart and never retries or expires. Confirmed takeover work replays its
original completion/cutoff until fresh authoritative acknowledgment, without repeating
removal; its confirmed durable floor survives acknowledgment and compaction. Maintenance
visits at most 16 wallets/pass and deletes at most 64 eligible receipts plus 64 attempts per
wallet while preserving floors, unknown and unacknowledged work. These bounds limit
mutations, not candidate scans or latency. A strict definite rejection can prepare a new
immutable attempt under current authority. Typed room-missing can permit ensuring the room
and rechecking; creation is never completion. Positively settled self-hosted quarantine
requires a full maximum 60-second issuer lifetime plus 60-second verifier leeway and
2-second safety allowance, including a fresh process's monotonic observation, before a
separately recorded final removal. Unknown final work stays closed.

Internal admission stamps carry exact wallet/session/realm/cluster/source/version, Pulse
floor, durable generation/floor and chosen credential `nbf` through existing assignment JSON
and socket queues. Configured queued credentials also retain their exact committed
TokenFence. These fields are private; client packets are unchanged. Every signing,
publication, refresh and dequeued-consumption boundary checks shared durable safety and
fresh Pulse authority. Configured final SQL authorization checks owner session/epoch/revision/token,
policy and current database-clock freshness after potentially blocking owner acquisition;
refresh includes the presented device. Queued output then checks current local owner/permit.
An old candidate never adopts a newer stamp or fence. All serves connect their existing
NATS link for nondestructive verification and signed refresh; replicas start no destructive
subscriber/job. Database or authority failure denies. These finite snapshots do not make
remote cleanup or physical network output atomic with later ownership/policy changes.

Lost hints and lost confirmed acknowledgments can recover while Pulse retains its ledger;
positively confirmed fenced recovery and replica refresh use the same durable safety.
Gatekeeper restart preserves hazards; it does not turn unknown outcomes into completion.
Unknown remote removal and Pulse process loss require controlled maintenance with admission
closed until outstanding operations, participants and token validity are safe. A new Pulse
incarnation does not revoke issued LiveKit tokens. Actual Cloud cutoff enforcement,
self-hosted expiry, deployment routing and rollout rehearsal remain separate evidence gates.
Consult the current readiness and measurement evidence before release.

## Scene host

As built, hosting is still `worlds::scene_state`: the old own-socket contract (`/ws/{scene}`,
one in-process V8 isolate per scene), which cannot run current SDK multiplayer scenes. The
decided target uses the room-participant contract (the server joins the scene's LiveKit room
as `authoritative-server`):
- `multiplayer-hosting`, a comms service run as one instance, starts a server on the first joined event for a scene with `authoritativeMultiplayer`, stops it 60 s after the room empties, reconciles against `GET /scene-participants`, keys servers by canonical room and entity hash (a redeploy drains the old one), allows 6 starts per 60 s, and pushes room tokens and 60-minute storage delegations to the engine. Metadata comes from `/content/entities/active` and `/content/contents/{hash}` (5 s timeout, verdict cached per entity hash). `/debug/*` answers only signed fetches from configured admin wallets, otherwise 403; local preview is deferred (503).
- The runtime is upstream bevy-explorer `headless --orchestrated`, unpatched and keyless, one engine per world. The one engine change left, as an upstream PR: report a room joined only once LiveKit confirms it.
- Presence, world capacity and the room-empty rule exclude `authoritative-server`.
- Pulse scene listeners must chain to the authoritative address, scoped to their parcels. Verify this requirement through the private acceptance review.

## Acceptance evidence

Read the architecture decisions, divergence findings and readiness evidence supplied for the task before planning or claiming deployment. Record missing implementation, measured capacity, live-client verification and required private security review there. Public ownership guidance does not itself establish readiness.
