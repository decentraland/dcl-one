# Worlds content server: comms handshakes, LiveKit webhook and presence

Area: the explorer comms handshakes of the worlds content server, its LiveKit webhook and the two
presence reads: `POST /worlds/:worldName/comms`, `POST /worlds/:worldName/scenes/:sceneId/comms`,
`POST /get-comms-adapter/:roomId`, `POST /livekit-webhook`, `GET /live-data`,
`GET /wallet/:wallet/connected-world`.

Pinned revisions:

- Upstream: `decentraland/worlds-content-server` @ `10af5255bb04a1a28e0c26bd190317c26a6015a9` (2026-10-21).
- dcl-one: origin/main `2da8b8aea` (2026-11-07), Rust workspace `services/catalyst`.

Everything below is from reading the code, except the rows marked `MATCH (fixed)`, which name
the dcl-one test that pins them. Nothing was captured from a live server. Three upstream packages
decide rows here. Their published tarballs were read but
never run:

- `@dcl/crypto-middleware` 6.3.0 (the signed-fetch verifier and `rejectIfSigner`);
- `@dcl/http-commons` 2.0.0 (`errorHandler` and the error classes);
- `livekit-server-sdk` 2.15.0 (`AccessToken`, `WebhookReceiver`).

`@dcl/http-server` (the router, CORS, 404/405) was not read. The rows that depend on it say so.

Files read:

- Upstream:
  - `src/controllers/routes.ts` (the three signed-fetch instances, `errorHandler`, route order)
  - `src/controllers/handlers/{world-comms-handler,comms-rate-limit-subject,comms-adapter-handler,livekit-webhook-handler,live-data-handler,wallet-connected-world-handler,world-about-handler}.ts`
  - `src/logic/comms/{component,errors,constants}.ts`
  - `src/logic/access/{component,types,constants}.ts`
  - `src/logic/access-checker/component.ts`
  - `src/logic/rate-limiter/{component,constants,errors}.ts`
  - `src/logic/client-source/component.ts`
  - `src/logic/worlds/component.ts` (`isWorldValid`, `getWorldSceneBaseParcelIncludingUndeployed`)
  - `src/logic/denylist/component.ts`
  - `src/adapters/{comms-adapter,peers-registry,bans-adapter,dcl-name-checker,name-deny-list-checker}.ts`
  - `src/adapters/livekit-client/{component,constants,types}.ts`
  - `src/adapters/worlds-manager.ts` (`getRawWorldRecords`, `getWorldScenes`)
  - `src/components.ts` (server CORS options)
  - `.env.default`
  - Tests:
    - `test/integration/{world-comms-handler,comms-adapter-handler,livekit-webhook-handler,live-data-handler,wallet-connected-world-handler,explorer-legacy-payload,strict-scene-signer-gate}.spec.ts`
    - `test/unit/{world-comms-handler,comms-component,comms-adapter,livekit-client,peers-registry,access-checker-component}.spec.ts`
    - `test/utils.ts` (`getLegacyAuthHeaders`)
- dcl-one (`services/catalyst/` unless the path says otherwise):
  - `comms/src/gatekeeper/admission/world_mint.rs` (the three mints)
  - `comms/src/signed_fetch.rs` (`require_verified`, `map_auth_error_api`)
  - `shared/src/signed_fetch.rs` (`SignedFetchError::http_status_and_message`)
  - `../../library/dcl-auth/src/signed_fetch.rs` (`is_refused_signer`, payload builders, `candidate_paths`)
  - `comms/src/worlds_envelope.rs` (`ApiError`, `envelope`)
  - `comms/src/gatekeeper/webhook.rs`, `comms/src/gatekeeper/errors.rs`, `comms/src/gatekeeper/mod.rs` (`webhook_router`)
  - `comms/src/presence/mod.rs`, `comms/src/presence/recovery.rs`
  - `comms/src/rooms.rs`, `worlds/src/rooms.rs` (`classify_room`, `resolve`, `world_scene`)
  - `comms/src/world_identity.rs` (`WorldName`)
  - `comms/src/access.rs`, `worlds/src/policy.rs` (`readmit`, `setting_access`, `world_admission`)
  - `comms/src/gatekeeper/admission/functions.rs` (`world_secret_refund`)
  - `comms/src/deny_list.rs`
  - `shared/src/livekit.rs`, `../../library/dcl-livekit-token/src/lib.rs`
  - `worlds/src/about.rs` (`resolve_fixed_adapter`)
  - `worlds/src/storage/auth.rs` (the upstream-exact signed-fetch messages the storage routes use)
  - `config/src/worlds.rs`, `config/src/social.rs`, `config/src/posture.rs`
  - `src/serve.rs` (mounts, around line 1340), `../../infra/gateway/{gateway.py,root-aliases.nix}`
  - `docs/worlds-compatibility.md`
  - Tests:
    - `tests/gatekeeper_policy.rs` (`the_world_mints_decide_through_the_permission_core_and_throttle_secrets`)
    - `src/cross_tests/presence_routes_tests.rs`
    - `comms/src/presence/recovery_tests.rs`
    - `world_mint.rs` `policy_boundary_tests`

Common notes:

- **Mount points.** Upstream serves every route of this area at the root of
  `worlds-content-server.decentraland.org`. dcl-one mounts:
  - the three mints at the root and under `/worlds-content-server` (`src/serve.rs:1357-1367`);
  - `/live-data` and `/wallet/{wallet}/connected-world` at both mounts, behind the worlds writes
    gate, which passes GETs (`src/serve.rs:1466-1482`);
  - the webhook at the root, under `/worlds-content-server` and under `/comms-gatekeeper`
    (`src/serve.rs`, WC-18).
- **Upstream error envelope.** `@dcl/http-commons` `errorHandler` (`routes.ts:168`) answers:
  - `InvalidRequestError` as 400 `{"error":"Bad request","message":m}`;
  - `NotFoundError` as 404 `{"error":"Not Found","message":m}`;
  - `NotAuthorizedError` as 401 `{"error":"Not Authorized","message":m}`;
  - anything else as 500 `{"error":"Internal Server Error"}`.

  `world-comms-handler` answers its own refusals as `{"error": m}` with no `message`.
- **dcl-one envelope.** `comms/src/worlds_envelope.rs:133-140`:
  - the http-commons labels for 400, 401 and 404;
  - the message in `error` (and in `message`) for 403;
  - `{"error":"Internal Server Error"}` for 500;
  - `Retry-After: 1` on any 503 without one (`worlds_envelope.rs:197-203`).

  The world mints rewrite 401, 403, 404, 429 and 503 to `{"error":m,"message":m}`.
  `/get-comms-adapter` rewrites only 429 (`world_mint.rs` `answer`). A signed-fetch refusal keeps
  the middleware's body on every route (WC-01). The extra `message` key is intended
  (worlds-compatibility.md, "Error envelope on 403 and 500").
- **Signed fetch.** Upstream builds the explorer instance with `canonicalMetadataKeys
  ['signer','intent','secret']`, `rejectIfSigner('decentraland-kernel-scene')` and an
  `onError` that answers `{error: err.message, message: 'This endpoint requires a signed fetch
  request. See ADR-44.'}` (`routes.ts:74-94,127`). dcl-one runs the same stages in the same
  order, through `comms/src/signed_fetch.rs:131-183` with `EXPLORER_METADATA_KEYS`:
  1. the chain;
  2. the timestamp, with a ±60 s window and an absent header read as `0`;
  3. the metadata parse;
  4. the scene gate, with canonical form and folded keys;
  5. the 6.x payload;
  6. the legacy payload, after the declared-key check and only after a 401.

  So the statuses match, and the bodies are the middleware's (WC-01, fixed).
- **CORS.** Upstream's server has `cors: {methods: [...], maxAge: 86400}` (`src/components.ts:101-104`;
  the preflight answer itself is `@dcl/http-server`'s, not read). The dcl-one mint, presence and
  webhook routers all carry `cors_middleware` and the NUL guard, so a preflight answers 204 with
  the CORS headers (WC-02, fixed).
- **Room names.** Upstream `.env.default`: `COMMS_ROOM_PREFIX=world-`,
  `SCENE_ROOM_PREFIX=world-scene-room-`.
  - Lobby: upstream `world-<lower(world)>`; dcl-one `world-<key>` with the key lowercased
    (`comms/src/rooms.rs:91-93`). The same.
  - Scene: upstream `world-scene-room-<lower(world)>-<lower(sceneId)>`
    (`src/adapters/comms-adapter.ts:183-186`); dcl-one `w1:<base36 len>:<key><scene id, case kept>`
    (`comms/src/rooms.rs:98-105`). Intended (worlds-compatibility.md, "Scene room names"; WC-14).
- **LiveKit token.** Both mint `livekit:<wss url>?access_token=<jwt>`, TTL 300 s, identity the
  lowercase signer. The `video` grant is the same:
  `roomJoin:true`, `room`, `roomList:false`, `canPublish`, `canSubscribe`, `canPublishData`,
  `canUpdateOwnMetadata`, `canPublishSources:["microphone"]`. Sources:
  - upstream `livekit-client/component.ts:52-65`, `comms-adapter.ts:130-141`;
  - SDK `AccessToken.toJwt` (`nbf: 0`, no `name` or `metadata`);
  - dcl-one `library/dcl-livekit-token/src/lib.rs:84-95,156-180`, `world_mint.rs:534-555`.

  dcl-one also sets `nbf` to now minus the clock skew and the participant attribute
  `dcl.worldAuthorization` (WC-13).

## Endpoint inventory

| # | method | upstream path | dcl-one route(s) | status |
|---|---|---|---|---|
| 1 | POST | `/worlds/:worldName/comms` | `/worlds/{world_name}/comms` and `/worlds-content-server/...` → `comms/src/gatekeeper/admission/world_mint.rs` (`world_comms` → `mint`) | DIVERGENT |
| 2 | POST | `/worlds/:worldName/scenes/:sceneId/comms` | `world_mint.rs` (`world_scene_comms` → `mint`) | DIVERGENT |
| 3 | POST | `/get-comms-adapter/:roomId` | `world_mint.rs` (`get_comms_adapter` → `mint`) | DIVERGENT |
| 4 | POST | `/livekit-webhook` | the root, `/worlds-content-server/livekit-webhook`, `/comms-gatekeeper/livekit-webhook` → `comms/src/gatekeeper/webhook.rs:191` | DIVERGENT (intended only) |
| 5 | GET | `/live-data` | both mounts → `comms/src/presence/mod.rs` (`live_data`) | DIVERGENT (intended only) |
| 6 | GET | `/wallet/:wallet/connected-world` | both mounts → `comms/src/presence/mod.rs` (`connected_world`) | DIVERGENT (intended only) |

Counts: 6 endpoints. MATCH 0, GAP 0, DIVERGENT 6 (3 of them on intended differences only),
MISSING 0. The three mints keep the decision-order differences of WC-09.

---

## 1–2. POST /worlds/:worldName/comms and /worlds/:worldName/scenes/:sceneId/comms

### Request

- Upstream: `explorerSignedFetchMiddleware` then `worldCommsHandler`
  (`src/controllers/handlers/world-comms-handler.ts:31-115`).
  - The secret is `authMetadata.secret`.
  - The scene id comes from the path. The handler reads no metadata `sceneId`.
- dcl-one: `mint(.., explorer_handshake=false)`.
  - `require_verified(.., EXPLORER_METADATA_KEYS)` (`world_mint.rs:356-364`).
  - The secret is `metadata.secret` when it is a string (`comms/src/signed_fetch.rs:43-50`).
  - The world name is parsed with `WorldName::parse`. A name with whitespace, `/` or a control
    character, or one that the trim changes, is an unknown world: 404, as upstream's
    `isWorldValid` answers it (WC-09).

### Decision order

| step | upstream (`logic/comms/component.ts:68-108`, handler) | dcl-one (`world_mint.rs` `mint`) |
|---|---|---|
| 1 | shared-secret world and subject throttled → 429 (`handler:46-52`) | signed fetch |
| 2 | platform ban (comms-gatekeeper `/users/:a/bans`, fail-open) → 401 | operator block list / platform ban (`admission_ban`) → 401 |
| 3 | deny list (`DENYLIST_JSON_URL`, 5 min cache, fail-open) → 401 | external deny list → 401 |
| 4 | `isWorldValid`: a row exists and the owner is not blocked for more than 2 days → else 404 | a name that is not a world key → 404 |
| 5 | name owner or `checkAccess` → else 401 (403 on a shared-secret world) | unknown or blocked world → 404 |
| 6 | scene: `getWorldSceneBaseParcelIncludingUndeployed` → else 404 | scene: `SceneId::parse` + `resolve(WorldScene)` (retired scenes included, `worlds/src/rooms.rs:125-143`) → else 404 |
| 7 | scene: comms-gatekeeper scene ban by base parcel (fail-open) → 401 | `can_access`: platform/device and scene bans (lobby too, WC-10), setting, then owner → 401/403 |
| 8 | capacity: lobby from the 60 s cached LiveKit status, scenes live, `>= MAX_USERS_PER_WORLD` (100) → 503 | throttle reservation only for an invalid or failed secret (`world_mint.rs:479-499`) → 429 |
| 9 | token | capacity from the live presence (`admission_occupancy`, ≤5 s sample), fail closed → 503 |
| 10 | clear the throttle on success | token + `dcl.worldAuthorization` attribute |

The name deny list: upstream `getRawWorldRecords` excludes banned names, but `getBannedNames`
strips `.dcl.eth` before comparing with full world names (`name-deny-list-checker.ts:51-54`,
`worlds-manager.ts:344-349`), so it does not refuse `.dcl.eth` worlds on these routes. dcl-one does
not consult it here either. MATCH in effect.

### Responses

| case | upstream | dcl-one | verdict |
|---|---|---|---|
| success | 200 `{"fixedAdapter":"livekit:wss://<host>?access_token=<jwt>"}` | 200 the same shape (`world_mint.rs:553-555`) | MATCH (token claims WC-13) |
| no auth chain | 400 `{"error":"Invalid Auth Chain","message":"This endpoint requires a signed fetch request. See ADR-44."}` (integration test, exact) | the same | MATCH (fixed, WC-01) |
| scene signer, non-canonical `signer`, folded `Signer` | 400 `{"error":"Invalid metadata content: <JSON, 64 chars + …>","message":ADR-44}` | the same | MATCH (fixed, WC-01) |
| malformed link | 400 `error` `Invalid chain format: malformed auth link at position N` | the same | MATCH (fixed, WC-01) |
| expired | 401 `error` `Expired signature: signature timestamp: T, timestamp expiration: T+60000, local timestamp: N` | the same | MATCH (fixed, WC-01) |
| bad signature, legacy `method:path` payload | 401 `error` `Invalid signature: <reason>` | 401 `Invalid signature: <this verifier's reason>` | MATCH (fixed, WC-01; reason text intended) |
| legacy payload, re-spelled declared key | 400 `error` `Invalid chain metadata: expected "secret", got "Secret"` | the same | MATCH (fixed, WC-01) |
| legacy payload, canonical keys; non-canonical `intent` in 6.x | 200 | 200 | MATCH |
| EIP-1654 chain | lambdas `validate-signature`; 503 `Error connecting to catalyst "<origin>": …` if it cannot be reached | `state.eip1654()` validator; 503 `Error connecting to catalyst: …` | DIVERGENT, intended (worlds-compatibility.md "Signed-fetch refusals") |
| unknown or blocked world | 404 `{"error":"World \"w\" is invalid or blocked."}` | 404 same text in `error` and `message` | MATCH (+`message`, intended) |
| world row without scenes | 200 | 200 (lobby canonical when the row exists, `worlds/src/rooms.rs:284-290`) | MATCH |
| neither owner nor access | 401 `{"error":"You are not allowed to access world \"w\"."}` | 401 same text | MATCH (+`message`) |
| shared secret wrong or missing | 403 `{"error":"You are not allowed to access world \"w\"."}` | the same (+`message`) | MATCH (fixed, WC-06) |
| throttled | 429 `{"error":"Too many shared-secret attempts. Try again later."}`, `Retry-After: 60` | 429 same text, `Retry-After: 60` | MATCH (+`message`) |
| deny-listed wallet | 401 `Access denied, deny-listed wallet.` | the same | MATCH (fixed, WC-08) |
| platform-banned | 401 `Access denied, platform-banned user.` | the same | MATCH (fixed, WC-07) |
| scene ban | 401 `You are banned from world "w".` | 401 same | MATCH |
| scene not in world | 404 `Scene "id" not found in world "w".` | 404 same | MATCH |
| at capacity | 503 `World "w" has reached its maximum capacity.` | 503 same, plus `Retry-After: 1` | MATCH (+header, WC-12) |
| access undetermined (DB, names authority, presence) | 500 `{"error":"Internal Server Error"}` (most faults fail open) | 503 `World access could not be decided; retry shortly.` | intended (worlds-compatibility.md "Owner admission", "Capacity check fails closed") |
| CORS preflight | `@dcl/http-server` CORS (not read) | 204 with `Access-Control-Allow-*` (`cors_middleware`) | MATCH (fixed, WC-02; upstream body not read) |

### Data dependency

- Upstream:
  - Access settings: `worlds.access` JSON.
  - Name owner: `nameOwnership` (marketplace subgraph or on-chain).
  - Communities: social service `getMemberCommunities`. A failure fails closed (401).
  - Bans and platform bans: comms-gatekeeper over HTTP, fail-open.
  - Deny list: `DENYLIST_JSON_URL`.
  - Occupancy: LiveKit `listRooms`.
  - Throttle: Redis.
- dcl-one:
  - Access settings: `worlds.worlds` (`access_type`, `access_secret_hash`), allow-list rows in
    `worlds.world_permissions`, communities in `social.community_members`. A failure is 503.
  - Name owner: the names authority.
  - Bans: `social.user_bans`, `social.scene_bans` and `worlds.blocked_wallets`.
  - Deny list: `DENY_LIST_URL` or `DENYLIST_JSON_URL`, Decentraland's under `NODE_POSTURE=mirror`
    (WC-22).
  - Occupancy: the presence recovered from LiveKit (WC-19).
  - Throttle: `social.world_secret_attempts`.

---

## 3. POST /get-comms-adapter/:roomId

### Request

- Upstream `comms-adapter-handler.ts:13-87`, in this order:
  1. signed fetch;
  2. `metadata.signer === 'dcl:explorer' && metadata.intent === 'dcl:explorer:comms-handshake'`,
     else 400 `Access denied, invalid metadata`;
  3. the room prefix (`COMMS_ROOM_PREFIX`), else 400 `Invalid room id requested.`;
  4. `isWorldValid`, else 404 `World "w" was not found.`;
  5. the shared-secret throttle, 429;
  6. owner or access, else 401 `You are not allowed to access world "w".`, on every access type;
  7. the token.

  No deny list, ban or capacity check.
- dcl-one `world_mint.rs` `get_comms_adapter` runs `mint(.., explorer_handshake=true)`: signed
  fetch, the same signer/intent equality, then the `world-` prefix (as upstream, WC-03), then the
  full world-mint pipeline above.

  The extra ban, deny-list and capacity checks are intended (worlds-compatibility.md).

### Responses

| case | upstream | dcl-one | verdict |
|---|---|---|---|
| success | 200 `{"fixedAdapter": <lobby token>}` | 200 the same | MATCH |
| unsigned | 400 ADR-44 body (exact test) | the same, whatever the room id | MATCH (fixed, WC-01, WC-03) |
| bad metadata | 400 `{"error":"Bad request","message":"Access denied, invalid metadata"}` | the same | MATCH |
| bad room prefix | 400 `{"error":"Bad request","message":"Invalid room id requested."}` | the same body, after the metadata check | MATCH (fixed, WC-03) |
| unknown world | 404 `{"error":"Not Found","message":"World \"w\" was not found."}` (test) | the same | MATCH (fixed, WC-04) |
| not allowed | 401 `{"error":"Not Authorized","message":"You are not allowed to access world \"w\"."}` | the same | MATCH |
| shared secret wrong | 401 Not Authorized (test `should use the IP as the rate limit subject` expects 401) | 401 `{"error":"Not Authorized","message":"You are not allowed to access world \"w\"."}` | MATCH (fixed, WC-05) |
| throttled | 429 `{"error":"Too many shared-secret attempts. Try again later."}`, `Retry-After: 60` | the same (+`message`) | MATCH |
| deny-listed, banned, full | 200 (no check) | 401 / 401 / 503 | intended (worlds-compatibility.md "Ban, deny-list and capacity on `/get-comms-adapter`") |

---

## 4. POST /livekit-webhook

### Request

- Upstream `livekit-webhook-handler.ts:17-71`:
  - The body is read as text.
  - An empty `Authorization` header is 400 `Authorization header not found`.
  - `WebhookReceiver.receive(body, auth)` checks the JWT and its `sha256` claim. It throws a
    plain `Error`, which `errorHandler` answers as 500 `{"error":"Internal Server Error"}`.
  - A missing `participant.identity` or `room.name` is 400.
  - Only `participant_joined` and `participant_left` on rooms whose name **ends with
    `.dcl.eth`** are applied. That is lobby rooms of DCL NAME worlds: scene rooms end with the
    scene id and ENS worlds end with `.eth`.
- dcl-one `comms/src/gatekeeper/webhook.rs:191-385`, the gatekeeper's single webhook:
  - It needs LiveKit configured.
  - It accepts `Authorization` or `x-livekit-signature`, verified with the same JWT and
    `sha256` rule (`shared/src/livekit.rs:173`).
  - It requires the event `id` and `createdAt`, and `participant.sid` on participant events.
  - It deduplicates by event id and orders participant events by SID and time.

### Responses and effects

| case | upstream | dcl-one | verdict |
|---|---|---|---|
| no Authorization | 400 `{"error":"Bad request","message":"Authorization header not found"}` | 401 `{"error":"invalid livekit webhook signature"}` | intended (worlds-compatibility.md "LiveKit webhook"; WC-16) |
| bad signature | 500 `{"error":"Internal Server Error"}` | 401 `{"error":"invalid livekit webhook signature"}` | intended (WC-16) |
| no identity / no room name | 400 `message` `Participant identity not found` / `Room name not found` | 400 `{"error":"missing livekit participant event fields"}` (participant events only) | intended (WC-16) |
| other event or room | 200 `{"message":"Skipping event"}` | 200 empty body | intended (WC-16) |
| join/leave applied | 200, body = the request body echoed | 200 empty body | intended (WC-16) |
| effect: NATS | `peer.<identity>.world.join` / `.leave` | `comms.room_events.v1` only | intended (worlds-compatibility.md "Room events"; WC-17) |
| effect: presence | the in-process `peersRegistry` (feeds `connected-world`) | invalidates the presence generation, which triggers a LiveKit re-read (~5 s); voice, streaming, room metadata and the access log run too | intended (worlds-compatibility.md "Presence from LiveKit"; WC-21) |

LiveKit only looks at the status, so the body differences reach no client. A status other than
2xx on a bad signature (500 here, 401 in dcl-one) is a failure either way.

---

## 5. GET /live-data

- Upstream `live-data-handler.ts:4-20` over `commsAdapter.status()`, with a one-entry LRU of 60 s
  (`comms-adapter.ts:220-240`):
  - `data.totalUsers` is the sum of `numParticipants` over LiveKit rooms named `world-*` and not
    `world-scene-room-*`, with users > 0.
  - `data.perWorld` is `[{worldName: room minus prefix, users}]` in LiveKit's order.
  - `lastUpdated` is the ISO time of the status fill.
  - On a LiveKit failure it answers zeros with `lastUpdated` = now (`comms-adapter.ts:165-175`).
- dcl-one `presence/mod.rs` `live_data` reads the recovered view (`recovery.rs`), polled every
  30 s, or 5–6 s after a webhook hint (`recovery.rs` `spawn_recovery`):
  - Lobby rooms only (`counts_of`, `presence/mod.rs:202-219`), counting identities other than
    `authoritative-server`.
  - `perWorld` is sorted by name.
  - `lastUpdated` is the last complete observation, `1970-01-01T00:00:00.000Z` before the first.
  - After 90 s without a complete observation, `data` is `{"totalUsers":0,"perWorld":[]}` with the
    old `lastUpdated` (`recovery_tests.rs:134-200`).

| field | upstream | dcl-one | verdict |
|---|---|---|---|
| shape | `{data:{totalUsers, perWorld:[{worldName, users}]}, lastUpdated}` | the same (`presence/mod.rs:367-397`) | MATCH |
| lobby-only count | yes | yes | MATCH |
| `lastUpdated` with no data | now | epoch, or the last success | intended (worlds-compatibility.md "Presence from LiveKit"; WC-20) |
| `perWorld` order | LiveKit list order | name order | intended (WC-20, harmless) |
| freshness | ≤60 s | ≤30 s, ~5 s after an event; empty after 90 s of failures | intended (WC-20) |
| one refused room in LiveKit | counted or ignored | left out; the observation publishes (`recovery_tests.rs` `a_refused_room_is_left_out_and_keeps_only_its_own_world_undetermined`) | MATCH (fixed, WC-19) |
| CORS | server CORS | `cors_middleware` | MATCH |

---

## 6. GET /wallet/:wallet/connected-world

- Upstream `wallet-connected-world-handler.ts:6-29`, `peers-registry.ts`:
  - The world comes from the in-process map the webhook feeds, so each replica has its own.
  - Only `.dcl.eth` lobby rooms are tracked (see §4).
  - The last join wins: a join to another world moves the peer.
  - The wallet is matched case-insensitively and echoed as given.
  - 200 `{"wallet": <as given>, "world": <lowercase world>}`.
  - 404 `{"error":"Not Found","message":"Wallet <w> is not connected to any world"}`.
- dcl-one `presence/mod.rs` `connected_world`, `get_peer_world`:
  - The world comes from the recovered LiveKit view, so all replicas agree.
  - Every world room counts, scene rooms and ENS worlds included.
  - Case-insensitive, `authoritative-server` excluded.
  - A wallet in rooms of two different worlds is in the one it joined last (LiveKit's
    `joinedAtMs`, else `joinedAt`); two joins at the same instant, or with no join time, answer
    503 `Connected world could not be decided; retry shortly.`
  - After 90 s without a complete observation, every wallet is 404.

| case | upstream | dcl-one | verdict |
|---|---|---|---|
| connected | 200 `{wallet, world}` | 200 the same shape | MATCH |
| not connected | 404 `{"error":"Not Found","message":"Wallet 0x… is not connected to any world"}` | the same | MATCH |
| `/wallet//connected-world` | 404 (router, `@dcl/http-server` body not read) | 404 (no route) | MATCH (body unread) |
| only in a scene room / an ENS world | 404 | 200 | intended (worlds-compatibility.md "Presence from LiveKit"; WC-21) |
| in two worlds at once | 200, the last joined | 200, the last joined (503 on a tie) | MATCH (fixed, WC-21) |
| right after a join | immediate (on this replica) | after the next observation (~5–35 s) | intended (WC-21) |

---

## `/world/:name/about` comms fields (owned by the world area)

Both trees advertise
`comms.adapter = fixed-adapter:signed-login:<baseUrl>/worlds/<lowercase name>/comms`, or
`fixed-adapter:offline:offline` (upstream `world-about-handler.ts:127-133`; dcl-one
`worlds/src/about.rs:319-333`). That path is route 1, and dcl-one serves it at both mounts, so
`/about` points at a route that exists. dcl-one also answers offline while the SFU is down
(intended, worlds-compatibility.md). For the world area to note: `baseUrl` must include the
`/worlds-content-server` prefix when the realm is reached through it, and
`about.rs` tests cover that case (`about.rs:535`).

## Explorer and LiveKit expectations

- **Explorers** (unity, godot, bevy):
  - They read `/about` and POST the signed-login URL with
    `{signer:'dcl:explorer', intent:'dcl:explorer:comms-handshake', isGuest, origin, secret?}`,
    signed in the legacy or the 6.x format.
  - They dial `fixedAdapter` and treat the room name as opaque. Both trees accept the same
    payloads and refuse the same ones with the same statuses and middleware bodies.
  - A browser-hosted explorer also needs the CORS preflight, which both trees answer (WC-02).
- **LiveKit:**
  - Same lobby room names, same token grants and TTL.
  - Scene rooms are named differently (WC-14).
  - One webhook URL in dcl-one handles worlds, gatekeeper and voice events. Pointing LiveKit
    at both `/worlds-content-server/livekit-webhook` and `/comms-gatekeeper/livekit-webhook`
    is safe: events are deduplicated by id (`webhook.rs:128-170`).
  - A LiveKit project shared with an upstream worlds-content-server: dcl-one leaves the upstream
    rooms out of its presence, and a world whose name such a room carries answers 503 on its
    mints while the room is open (WC-19).

## Backlog (WC-)

1. **WC-01 · routes 1–3 · signed-fetch refusal bodies.** Status: MATCH (fixed).
   - Upstream: `{error: <crypto-middleware message>, message: "This endpoint requires a signed fetch
     request. See ADR-44."}` with the middleware's status (`routes.ts:89-92`; `@dcl/crypto-middleware`
     6.3.0 `verify.js`). The messages:
     - `Invalid Auth Chain`
     - `Invalid chain format: …`
     - `Invalid chain timestamp: …`
     - `Expired signature: signature timestamp: T, timestamp expiration: T+60000, local timestamp: N`
     - `Signature timestamp is too far in the future: …`
     - `Invalid chain metadata: "…"` / `expected "k", got "K"` / `"k" delivered under N spellings`
     - `Invalid metadata content: <JSON.stringify(metadata), 64 + "…">`
     - `Invalid signature: <reason>`
     - the 503 `Error connecting to catalyst "<origin>": …`
   - dcl-one: `map_auth_error_api` (`comms/src/signed_fetch.rs:187-190`) renders
     `SignedFetchError::http_status_and_message` (`shared/src/signed_fetch.rs:76-88`) through the
     worlds envelope. The world routes then rewrite 401 and 503 to `{error:m, message:m}`
     (`world_mint.rs:318-332`). The results:
     - 400 `{"error":"Bad request","message":"Invalid Auth Chain[: detail]"}`;
     - 401 `Unauthenticated: Expired signature` or `Unauthenticated: Invalid signature`.

     Truncation also differs: `...` here, `…` upstream.
   - Proposed fix:
     - Give `comms/src/signed_fetch.rs` its own refusal type that renders the upstream message
       text and the ADR-44 `message`. The text exists already for the storage routes
       (`worlds/src/storage/auth.rs:59-140` `AuthChainError::raw_message`). Add the scene-gate
       text `Invalid metadata content: <serialized metadata>` and the declared-key texts.
     - Have `answer()` return that body untouched for auth refusals.
     - Pin each upstream integration case (`world-comms-handler.spec.ts:198-520`,
       `explorer-legacy-payload.spec.ts`) in `tests/gatekeeper_policy.rs`.
   - Fixed: `require_verified` refuses with `SignedFetchRefusal` (the shared cause, which keeps the
     status, and the crypto-middleware 6.3.0 text, `…` truncation included), and
     `map_auth_error_api` answers `ApiError::SignedFetch`: `{error: <text>, message:
     SIGNED_FETCH_MESSAGE}` (`comms/src/worlds_envelope.rs`, the constant the storage routes
     already used). `answer()` leaves that body alone. The mapper's only callers are worlds
     routes (the mints, settings, the eight signed permission routes, scene delete, undeploy,
     `/wallet/contribute`); no other route used it. Pinned by
     `tests/gatekeeper_policy.rs` `the_world_mints_decide_through_the_permission_core_and_throttle_secrets`
     (no chain on all three routes, malformed link, expired), `comms/src/signed_fetch.rs` tests and
     `worlds/src/core/auth.rs` tests (key guard, scene gate, absent timestamp). Left, intended:
     the `Invalid signature:` reason, the catalyst origin in the EIP-1654 503, and the 400 for a
     misplaced SIGNER or an unsigned later link (worlds-compatibility.md "Signed-fetch refusals").
   - Severity: P2 (explorers read only the status).

2. **WC-02 · routes 1–3 · no CORS, so a browser preflight gets 405.** Status: MATCH (fixed).
   - Upstream: the server's CORS (`src/components.ts:101-104`). The preflight answer is
     `@dcl/http-server`'s, not read.
   - dcl-one: `world_mint::router` has no `cors_middleware` (`world_mint.rs:256-265`), unlike the
     presence router (`presence/mod.rs:343-350`) and `webhook_router` (`gatekeeper/mod.rs:916-923`).
     `serve` adds no global CORS. The handshake carries `x-identity-*` headers, so a browser
     always sends a preflight.
   - Proposed fix: add `.layer(from_fn(nul_guard_middleware)).layer(from_fn(cors_middleware))`
     to `world_mint::router`, and a preflight test for both mounts in `tests/gatekeeper_policy.rs`.
   - Fixed as proposed. Pinned by `tests/gatekeeper_policy.rs`
     `the_mints_answer_a_cors_preflight_at_both_mounts`.
   - Severity: P1 (a web-hosted explorer cannot join a world; desktop clients are unaffected).

3. **WC-03 · `/get-comms-adapter` · the room-prefix check runs before the signed fetch.** Status: MATCH (fixed).
   - Upstream: signed fetch, then metadata, then prefix (`comms-adapter-handler.ts:24-33`).
   - dcl-one: `strip_prefix(WORLD_ROOM_PREFIX)` runs before `mint` (`world_mint.rs:288-290`). An
     unsigned request, or one with bad metadata, to `/get-comms-adapter/foo` gets
     `Invalid room id requested.` instead of the ADR-44 400 or `Access denied, invalid metadata`.
   - Proposed fix: pass the raw room id into `mint` and check the prefix after
     `require_explorer_handshake`.
   - Fixed as proposed; pinned in `the_world_mints_decide_through_the_permission_core_and_throttle_secrets`.
   - Severity: P2.

4. **WC-04 · `/get-comms-adapter` · the 404 message.** Status: MATCH (fixed).
   - Upstream: `World "w" was not found.` (`comms-adapter-handler.ts:37-39`; the integration test
     asserts it).
   - dcl-one: the mints' `World "w" is invalid or blocked.` (`world_mint.rs:151-153,417-419`).
   - Proposed fix: let `invalid_world` take the route, and answer the adapter text when
     `explorer_handshake` is set.
   - Fixed as proposed; pinned in `tests/gatekeeper_policy.rs` and
     `comms/src/gatekeeper/tests.rs` `world_mints_answer_404_for_an_unknown_or_blocked_world`.
   - Severity: P2.

5. **WC-05 · `/get-comms-adapter` · a wrong or missing shared secret answers 403.** Status: MATCH (fixed).
   - Upstream: 401 `{"error":"Not Authorized","message":"You are not allowed to access world \"w\"."}`
     for every refusal (`comms-adapter-handler.ts:59-71`).
   - dcl-one: `Access::InvalidSecret` becomes 403 on both routes (`world_mint.rs:512-516`).
   - Proposed fix: when `explorer_handshake`, map `InvalidSecret` to the 401 `NotAllowed` text.
   - Fixed as proposed; pinned in `the_world_mints_decide_through_the_permission_core_and_throttle_secrets`.
   - Severity: P1 (an upstream integration test pins the 401 on this flow).

6. **WC-06 · routes 1–2 · the 403 message for a shared-secret refusal.** Status: MATCH (fixed).
   - Upstream: the `InvalidAccessError` text `You are not allowed to access world "w".` as 403
     `{error}` (`world-comms-handler.ts:72-84`, `logic/comms/errors.ts:22-27`).
   - dcl-one: `Access denied, invalid secret for world "w".` (`world_mint.rs:512-516`).
   - Fixed: the `InvalidAccessError` text; pinned in `tests/gatekeeper_policy.rs`.
   - Severity: P2.

7. **WC-07 · routes 1–3 · the platform-ban message.** Status: MATCH (fixed).
   - Upstream: `Access denied, platform-banned user.` (`logic/comms/errors.ts:50-55`; tests assert
     `toContain('platform-banned')`).
   - dcl-one: `Access denied, you are banned from the platform.` for the block list and platform
     bans (`world_mint.rs:410-414,507-511`).
   - Fixed: the upstream text for the block list, platform bans and `Access::Banned`; pinned in
     `tests/gatekeeper_policy.rs` (a blocked wallet).
   - Severity: P2.

8. **WC-08 · routes 1–3 · the deny-list message.** Status: MATCH (fixed).
   - Upstream: `Access denied, deny-listed wallet.` (`logic/comms/errors.ts:36-41`).
   - dcl-one: `Access denied, wallet is deny-listed.` (`world_mint.rs:369-373`).
   - Fixed: the upstream text (no test drives a deny-list document here).
   - Severity: P2.

9. **WC-09 · routes 1–2 · decision order.** Status: open, partly fixed.
   - Upstream:
     1. the throttle pre-check (shared-secret worlds);
     2. platform ban;
     3. deny list;
     4. world valid;
     5. owner or access;
     6. scene exists;
     7. scene ban;
     8. capacity.
   - dcl-one: deny list, name parse, ban, world valid, **scene resolve**, access (bans, setting,
     owner), throttle reservation, capacity (`world_mint.rs:369-533`). Combinations that answer
     differently:
     - a platform-banned wallet on a throttled shared-secret world (upstream 429, here 401);
     - a wallet without access asking for a missing scene (upstream 401, here 404);
     - a padded or slashed world name (upstream 404, here 400 `Invalid world name`).
   - Proposed fix: reorder `mint`: throttle check first (a non-charging read), then platform
     ban, deny list, world, access, scene, scene ban, capacity. Keep the reservation where it is.
   - Fixed: platform ban now runs before the deny list, and a name that is not a world key is
     404 (`invalid_world`) after both, as upstream.
   - Left: the throttle pre-check (a throttled caller still reaches the ban checks, and an owner
     on a throttled shared-secret world is admitted where upstream answers 429), and access
     before the scene (a wallet without access asking for a missing scene gets 404). Both need
     `can_access` split into the access setting and the scene ban, a change to the one
     permission core the scene host also uses.
   - Severity: P2.

10. **WC-10 · route 1 (and 3) · scene bans refuse the lobby.** Status: kept: intended (worlds-compatibility.md "Scene bans in the lobby").
    - Upstream: the lobby checks no scene ban. Only the scene route checks the ban of that
      scene's base parcel (`logic/comms/component.ts:93-108`).
    - dcl-one: `readmit` checks the world key, the active scene's base-parcel key and the legacy
      entity key for the lobby too (`worlds/src/policy.rs:1143-1188`). A wallet banned from the
      world's scene gets 401 `You are banned from world "w".` on `/comms` and `/get-comms-adapter`.
    - Proposed fix: keep (one ban model across rooms). Add it to `docs/worlds-compatibility.md`,
      since it is not listed there.
    - Severity: P2.

11. **WC-11 · routes 1–3 · a right secret does not clear earlier failures.** Status: MATCH (fixed).
    - Upstream: `clearAttempts` deletes the subject's attempts on success
      (`rate-limiter/component.ts:98-101`, `world-comms-handler.ts:61-63`).
    - dcl-one: `settle(false, …)` refunds only the request's own reservation
      (`world_mint.rs:96-116`, `functions.rs:28-44`). After two wrong secrets and a right one, the
      next wrong secret is the third of the window and the fourth attempt is 429. Upstream
      allows three more. `docs/worlds-compatibility.md` says "the count is cleared by a right
      secret", which the code does not do.
    - Proposed fix: on `Access::Allowed(SharedSecret)`, delete the `(world, subject)` row. Or
      correct the doc sentence.
    - Fixed in code, as upstream does: a mint admitted by the shared secret deletes the
      `(world, subject)` row once the token is minted (`SecretThrottle::clear`). Upstream also
      clears on an owner's join to a shared-secret world; that is listed in
      worlds-compatibility.md "Shared-secret throttle". Pinned in
      `the_world_mints_decide_through_the_permission_core_and_throttle_secrets` (two wrong, one
      right, then three more 403 before the 429).
    - Severity: P2.

12. **WC-12 · routes 1–3 · envelope extras.** Status: kept: intended (worlds-compatibility.md "Error envelope on 403 and 500").
    - The extra `message` key on 401/403/404/429/503 is intended (worlds-compatibility.md).
    - The 503 answers also gain `Retry-After: 1` (`worlds_envelope.rs:197-203`), where upstream
      sends none on its 503.
    - Proposed fix: keep, intended. A client reading only `error` gets the upstream text where
      WC-06..08 are fixed.
    - Severity: P2.

13. **WC-13 · routes 1–3 · token claims.** Status: kept: intended (worlds-compatibility.md "Token claims").
    - Upstream: `{video, iss, exp: now+300, nbf: 0, sub}` (SDK `AccessToken.toJwt`). The URL host
      is `LIVEKIT_HOST` as given when it has a scheme, else `wss://` + host
      (`livekit-client/component.ts:77-93`).
    - dcl-one:
      - `nbf` is now minus the clock skew;
      - `attributes: {"dcl.worldAuthorization": "{\"worldAuthorization\":{\"at\":…,\"signature\":…}}"}`,
        which other participants in the room can see;
      - the scheme is mapped `https`→`wss` and `http`→`ws` and trailing `/` is dropped
        (`shared/src/livekit.rs:63-90`).

      Identity, grants, TTL and room are the same.
    - Proposed fix: keep. The attribute carries dcl-one's re-admission proof.
    - Severity: P2.

14. **WC-14 · route 2 · scene room names.** Status: intended (worlds-compatibility.md "Scene room names").
    - Upstream: `world-scene-room-<lower(world)>-<lower(sceneId)>`.
    - dcl-one: `w1:<base36 len>:<key><scene id>`, with case preserved.
    - Drop-in impact: explorers treat the name as opaque. Tools that parse LiveKit room names
      differ:
      - an upstream service sharing the LiveKit project;
      - dashboards;
      - comms-gatekeeper's scene-room parsing.
    - Proposed fix: keep, intended.
    - Severity: P2.

15. **WC-15 · all · no `COMMS_ROOM_PREFIX`, `SCENE_ROOM_PREFIX`, `COMMS_ADAPTER=ws-room`, `COMMS_FIXED_ADAPTER`.** Status: intended (worlds-compatibility.md "LiveKit only", "Scene room names").
    - Upstream tests run on ws-room (`fixedAdapter: ws-room:…/rooms/world-<w>`); production uses
      LiveKit.
    - Proposed fix: keep.
    - Severity: P2.

16. **WC-16 · `/livekit-webhook` · statuses and bodies.** Status: kept: intended (worlds-compatibility.md "LiveKit webhook").
    - Upstream:
      - 400 `Authorization header not found`;
      - 500 on a bad signature;
      - 400 `Participant identity not found` / `Room name not found`;
      - 200 `{"message":"Skipping event"}`;
      - 200 echoing the body (`livekit-webhook-handler.ts:33-70`).
    - dcl-one:
      - 401 `{"error":"invalid livekit webhook signature"}` for a missing or bad signature;
      - 400 `missing livekit participant event fields` (it also requires `id` and `createdAt`);
      - 200 with an empty body (`webhook.rs:196-264,384`).
    - Proposed fix: keep. Only LiveKit calls this route and it reads only the status. If the
      upstream tests are to be mirrored, add the four texts and the skip body in `webhook.rs`.
    - Severity: P2.

17. **WC-17 · `/livekit-webhook` · NATS `peer.<identity>.world.join|leave` is not published.** Status: intended (worlds-compatibility.md "Room events").
    - Upstream: `nats.publish('peer.<identity>.world.<join|leave>')` for `.dcl.eth` lobby rooms
      (`livekit-webhook-handler.ts:62`).
    - dcl-one: `comms.room_events.v1` only.
    - Drop-in impact: a consumer outside this tree on the same NATS that listens for the old
      subjects gets nothing. None exists in this tree.
    - Proposed fix: keep. Add a compatibility publisher only if such a consumer is attached.
    - Severity: P2.

18. **WC-18 · `/livekit-webhook` · no root route in the binary.** Status: MATCH (fixed).
    - Upstream: `POST /livekit-webhook` at the root.
    - dcl-one:
      - served under `/worlds-content-server` and `/comms-gatekeeper` (`src/serve.rs:1360-1375`);
      - the root path exists only as the gateway alias `= /livekit-webhook` →
        `/comms-gatekeeper$uri` (`infra/gateway/root-aliases.nix:102-108`).

      A node run without that gateway answers 404 at the root.
    - Proposed fix: merge `webhook_router` at the root in `src/serve.rs` behind the same social
      writes guard. The guard keeps `/livekit-webhook` open (`social/src/writes.rs:38-46`). Or
      document the gateway requirement.
    - Fixed as proposed (`src/serve.rs`). The gateway alias still sends the root path to
      `/comms-gatekeeper`, the same handler. No test builds the whole `serve` router.
    - Severity: P2.

19. **WC-19 · presence (routes 1–3, 5, 6) · one refused LiveKit room fails the whole observation.** Status: MATCH (fixed).
    - Upstream: `/live-data` and the capacity read take whatever LiveKit lists, prefix-filtered
      (`comms-adapter.ts:144-176`).
    - dcl-one: `recover_complete` fails with `Unresolved` when any listed room classifies as
      `RelevantRefused` (`presence/recovery.rs:124-170`). `classify_room` refuses
      (`worlds/src/rooms.rs:336-420`):
      - a `world-`/`scene-` room whose world has no catalogue row;
      - an upstream-format `world-scene-room-…` room;
      - an uppercase or ambiguous name.

      Until such a room closes, the effects are:
      - `/live-data` empties after 90 s;
      - every `connected-world` answers 404;
      - every mint answers 503, because capacity fails closed.
    - Proposed fix: in `recovery.rs`, leave a refused room out of the counts (log it, with a
      metric) instead of failing the whole inventory. Fail a scope only when the refused room
      could belong to the world being counted, that is when its decoded world key equals the
      scope's. Add a test with a stray `world-unknown.dcl.eth` room in `recovery_tests.rs`.
    - Fixed: `inventory` leaves a refused room (and a canonical room delivered under another
      spelling) out, and fails only a scope it could hold (`Scope::may_hold`: the exact room, a
      `w1:` room decoding to the world, or a `world-`, `scene-` or `world-scene-room-` name
      naming the world). The published observation keeps the refused names, so the admission
      read from it also falls back to a scoped read that refuses that world. The count is the
      `comms_presence_refused_rooms` gauge, with a warning naming up to five rooms. Pinned by
      `comms/src/presence/recovery_tests.rs`
      `a_refused_room_is_left_out_and_keeps_only_its_own_world_undetermined` (a stray
      `world-unknown.dcl.eth`, upstream-format, malformed, ambiguous, legacy and non-canonical
      rooms).
    - Severity: P1 (one orphan or foreign room blocks admission to every world).

20. **WC-20 · `/live-data` · freshness and `lastUpdated`.** Status: kept: intended (worlds-compatibility.md "Presence from LiveKit").
    - Upstream:
      - `lastUpdated` is the cache fill time: now on a LiveKit failure, so never epoch;
      - 60 s cache;
      - LiveKit list order;
      - counts every participant.
    - dcl-one (`presence/mod.rs:400-421`):
      - epoch before the first observation;
      - the last success time with empty data after 90 s;
      - ≤30 s poll;
      - name order;
      - `authoritative-server` excluded.
    - Proposed fix: keep. A client reading `lastUpdated` as "now" can tell from the epoch that
      no data is in yet.
    - Severity: P2.

21. **WC-21 · `/wallet/:wallet/connected-world` · source and rules.** Status: MATCH (fixed) for the winner; the rest kept: intended (worlds-compatibility.md "Presence from LiveKit").
    - Upstream: the webhook-fed per-replica map (`peers-registry.ts:69-128`):
      - `.dcl.eth` lobby rooms only (the webhook's suffix filter);
      - the last join wins;
      - updated at once on the replica that got the webhook.
    - dcl-one: the recovered LiveKit view (`presence/mod.rs:176-199`):
      - scene rooms and ENS worlds included;
      - 503 when the wallet is in two worlds;
      - updated on the next observation (~5–35 s after the event);
      - consistent across replicas.
    - Proposed fix:
      - Pick the upstream winner instead of the 503: keep the latest join time per identity,
        which the webhook coordinator already tracks (`webhook.rs` `current`).
      - Keep counting scene rooms. It answers "which world" better, and a wallet in a scene
        is in that world.
    - Fixed: the recovered roster keeps LiveKit's `joinedAtMs` (else `joinedAt`), and
      `get_peer_world` answers the world of the latest join; a tie (or no join time) is still 503.
      Pinned by `recovery_tests.rs` `a_wallet_in_two_worlds_is_in_the_one_it_joined_last` and
      `shared/src/livekit.rs` `complete_empty_is_distinct_from_invalid_and_duplicate_inventory`.
      Scene rooms, ENS worlds and the observation delay are kept.
    - Severity: P2.

22. **WC-22 · routes 1–3 · the external deny list is off by default.** Status: MATCH (fixed) under `NODE_POSTURE=mirror`; off without a posture, intended (worlds-compatibility.md "Wallet deny list").
    - Upstream: `DENYLIST_JSON_URL` is required, default `https://config.decentraland.org/denylist.json`
      (`.env.default`, `logic/denylist/component.ts:23`).
    - dcl-one: `DENY_LIST_URL` with no default (`config/src/worlds.rs:410-412`). No posture sets
      it; `NODE_POSTURE=mirror` sets only `EXPLORER_DENYLIST_SOURCE_URL`
      (`config/src/posture.rs:241-245`). Unset, the check passes everyone (`comms/src/deny_list.rs:53-56`).
    - Proposed fix: read `DENYLIST_JSON_URL` as an alias of `DENY_LIST_URL`, and default it to the
      Decentraland document under `NODE_POSTURE=mirror` (`config/src/posture.rs`).
    - Fixed as proposed: `WorldsConfig::deny_list` reads `DENY_LIST_URL`, else `DENYLIST_JSON_URL`;
      the mirror posture sets `DENY_LIST_URL` to `DENYLIST_JSON_URL` when that is set, else to
      `https://config.decentraland.org/denylist.json`. The archipelago socket reads the same list.
    - Severity: P2.

23. **WC-23 · routes 1–3 · access-store faults.** Status: kept: intended (worlds-compatibility.md "Access stores fail closed").
    - Upstream:
      - a social-service failure on a community allow-list fails closed as 401
        (`access-checker-component.spec.ts:248-260`);
      - ban, platform-ban and deny-list lookups fail open (`bans-adapter.ts:82-87,112-118`,
        `denylist/component.ts:37-40`);
      - Redis faults fail open.
    - dcl-one: any store fault is 503 `World access could not be decided; retry shortly.`
      (`world_mint.rs:334-337`); a throttle-counter fault is 503. Intended for owner admission,
      capacity and the throttle (worlds-compatibility.md). The community and ban cases are not
      listed there.
    - Proposed fix: keep fail-closed, and list the community, ban and scene-ban stores in
      `docs/worlds-compatibility.md`.
    - Severity: P2.

24. **WC-24 · routes 1–3 · shared-secret throttle mechanics.** Status: intended (worlds-compatibility.md "Shared-secret throttle").
    - Upstream: `cf-connecting-ip` (`TRUSTED_CLIENT_IP_HEADER`), else the wallet; a sliding
      60 s window of timestamps in Redis; fails open.
    - dcl-one: `X-Real-IP`, IPv6 by /64, else the wallet; a fixed window from the first
      attempt in Postgres; 503 on a counter fault. Both answer 403, 403, 403, 429 to four wrong
      secrets, and 429 to a right secret while throttled (`tests/gatekeeper_policy.rs`).
    - Proposed fix: keep.
    - Severity: P2.

25. **WC-25 · routes 1–3 · signed-fetch statuses and the legacy payload.** Status: MATCH, no work.
    - Upstream: `verify()` stage order; legacy fallback only after a 401 and only with every
      declared key canonically spelled; `rejectIfSigner` refuses a non-canonical `signer` and any
      folded `Signer` before the signature (`@dcl/crypto-middleware` 6.3.0 `verify.js`,
      `metadataValidators.js`).
    - dcl-one: the same stages, keys and refusals (`comms/src/signed_fetch.rs:131-183`,
      `library/dcl-auth/src/signed_fetch.rs:185-208`). Bodies are WC-01.
    - Severity: none (closed).

26. **WC-26 · routes 1–2 · undeployed scenes and world rows without scenes are admitted.** Status: MATCH, no work.
    - Upstream: `getWorldSceneBaseParcelIncludingUndeployed`; `isWorldValid` needs only the row.
    - dcl-one: `world_scene` unions `worlds.retired_world_scenes` (`worlds/src/rooms.rs:125-143`); a
      world row admits the lobby.
    - Severity: none (closed).

27. **WC-27 · `/world/:name/about` · the comms adapter points at route 1.** Status: MATCH, no work.
    - Both trees: `fixed-adapter:signed-login:<baseUrl>/worlds/<lower>/comms`; dcl-one adds the
      SFU-down offline answer (intended).
    - Severity: none (closed). The world area owns `/about`.

## Endpoints dcl-one serves in this area that upstream does not

- The same three mints, the webhook and the two presence reads under `/worlds-content-server/…`
  (a second mount of the routes above).
- `POST /comms-gatekeeper/livekit-webhook`: the same handler. It also drives voice chats,
  ingress streams, room metadata and the worlds access log.
- `POST /internal/pulse/room-authority/v1` at the root (`gatekeeper::room_authority_router`), when a
  service key is configured. It is the Pulse room authority, not a worlds route.
