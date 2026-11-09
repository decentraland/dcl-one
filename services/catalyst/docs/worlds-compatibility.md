# Worlds: intended differences from worlds-content-server

The `worlds` crate serves the worlds content server's HTTP surface (under
`/worlds-content-server`, and at the root where no catalyst route has the
path; see "Mounts") and its comms admission. The wire
contract follows
[`decentraland/worlds-content-server`](https://github.com/decentraland/worlds-content-server).
This page lists where the catalyst answers differently on purpose. A
difference not listed here is a gap, not a decision.

## HTTP

- **Invalid world names.** The permission, settings and manifest routes and
  both undeploy routes (`DELETE /entities/{name}`,
  `DELETE /world/{name}/scenes/{coordinate}`) parse the world name first
  and answer 400 `Invalid world name: …` for a name that is not a valid
  world key, before the signed fetch is checked. The upstream service
  answers defaults or 404 for such names. A scene undeploy coordinate must
  fit a 32-bit integer (400 otherwise; upstream checks the pattern only),
  and that route answers 408 `request timed out` after 30 seconds.
- **Wallet storage allowance.** `GET /wallet/{wallet}/stats`, the publish
  size cap and `GET /world/{name}/limits` count only scenes published here:
  mirrored worlds and scenes are left out. Personal worlds are outside the
  allowance and are not listed, nor are bare-name worlds (upstream lists
  them under `ensNames`). A wallet's DCL NAME worlds are the NAMEs it
  holds now (the stored owner only without a names reader). A DCL NAME
  world's publish is capped at 500 MiB per scene (the upstream
  `MAX_SCENE_SIZE` default; not configurable here) and by what the owner's
  allowance leaves; the publish re-checks the allowance in its transaction,
  so concurrent publishes cannot exceed it together. The allowance is
  computed in process, as dcl-name-stats' `account-holdings` does: 100 MiB
  per DCL NAME (registrar `balanceOf`), per LAND (`balanceOf`) and per
  estate LAND (`getLANDsSize`), plus 100 MiB per whole 2000 MANA held
  across Ethereum and Polygon, read with `eth_call` at `latest` over
  `ETH_RPC_URL` (one batch) and `RPC_ENDPOINT_POLYGON` for `ETH_NETWORK`
  (mainnet with Polygon, sepolia with Amoy). Unlike upstream, which counts a
  failed or reverted call as zero, any failed call is an allowance outage.
  Without either RPC URL, or on another network, the allowance is unlimited
  (logged at startup) and `maxAllowedSpace` reports the per-scene caps of
  the wallet's DCL NAME worlds; the stats route reports the same during an
  outage, while the publish and `/limits` answer 503 and the quota pass
  keeps existing blocks. An answered allowance is reused for 60 seconds and
  a failed lookup for 15 seconds. At most 8 publish and `/limits` lookups
  run at once, and one that waits over 5 seconds for a slot answers 503;
  the stats route has 2 slots of its own and, when they are busy, reports
  the per-scene caps instead of waiting. The 12-hourly quota pass evaluates
  both the stored owner and the live holder of each DCL NAME world.
  A publish credits back only the replaced scenes of a world charged to
  the same owner.
- **Legacy content keys.** The worlds `/contents/{key}` route also accepts a
  64-hex sha256 key and serves the blob whose raw-leaf CIDv1 has that
  digest, which covers files of one chunk (256 KiB) or less. It also serves
  CIDv0 (`Qm…`) keys. An invalid key answers 404 without a body, as a miss
  does (upstream: 400). A key on the content denylist answers 404, and with
  `READ_THROUGH_ENABLED` a missing key is fetched from the sync sources
  before the 404.
- **Content types.** The worlds `/contents` and `/ipfs` routes sniff the
  file's first 64 bytes with `file-type@16`'s signatures for images (PNG,
  JPEG, GIF, WebP, BMP, AVIF, HEIC), glTF binary, fonts, the ISO-BMFF brands,
  MP3, WAV, FLAC, Ogg, WebM, ZIP and WebAssembly; a format that needs more of
  the file is `application/octet-stream`. A file that opens with `{` or `[`
  is `application/json` (upstream: `application/octet-stream`).
- **Conditional reads.** A request whose `If-None-Match` names the ETag gets
  304 on `/contents`, `/ipfs` and `/index`; upstream always answers 200.
- **Error envelope on 403 and 500.** Worlds routes answer
  `{error, message}` with the @dcl/http-commons labels (`Bad request`,
  `Not Authorized`, `Not Found`). A 403 carries the message in `error` as
  the upstream handlers' own 403s do, and also in `message`. A 500 is
  `{"error": "Internal Server Error"}`. The world comms mints
  (`/worlds/{name}/comms`, `/worlds/{name}/scenes/{id}/comms`) carry the
  reason in `error` for 401, 403, 404, 429 and 503, as upstream, and also in
  `message`; `/get-comms-adapter` does so for 429 only. Every 503 carries
  `Retry-After: 1` unless it names its own wait.
- **Signed-fetch refusals** answer upstream's middleware body,
  `{"error": <@dcl/crypto-middleware 6.3.0 message>, "message": "This
  endpoint requires a signed fetch request. See ADR-44."}`, at its status.
  The reason after `Invalid signature: ` is this node's verifier's, and the
  catalyst 503 does not name the catalyst's origin. A chain whose SIGNER link
  is not first, or whose later link has no signature, is refused at
  extraction (400 `Invalid chain format: malformed auth link at position N`),
  where upstream answers 401 `Invalid signature: …`.

## Comms

- **Operator disables apply at once.** An operator disable
  (`/admin/worlds/{name}/disable`) refuses the world on `/about` and the
  comms mints immediately. A storage-quota block keeps the upstream two-day
  grace and refuses only the owner's local DCL NAME and ENS worlds, not its
  personal world or mirrored worlds. The block follows the NAME's live
  holder: a sold world is judged by its new holder's block (the stored
  owner's while the authority cannot answer).
- **Shared-secret throttle.** Attempts are counted in Postgres per world
  and client source (`X-Real-IP`, IPv6 by /64; the wallet without one) over
  a fixed 60-second window from the first attempt
  (`SHARED_SECRET_MAX_ATTEMPTS_PER_MINUTE`). Each attempt is reserved before
  the secret is checked and the count is cleared by a right secret
  (upstream also clears it when the owner joins). If the counter cannot be
  written the mint answers 503.
- **Scene bans in the lobby.** A wallet banned from the world's active scene
  is refused the world lobby too (`/worlds/{name}/comms` and
  `/get-comms-adapter`, 401 `You are banned from world "<name>".`);
  upstream checks a scene ban only on the scene route.
- **Access stores fail closed.** When the access setting, a listed
  community's membership, a platform, wallet or scene ban, or the throttle
  counter cannot be read, the mint answers 503 `World access could not be
  decided; retry shortly.`. Upstream lets ban, deny-list and throttle faults
  through and refuses a community fault with 401.
- **Wallet deny list.** The mints read `DENY_LIST_URL` (or upstream's name
  `DENYLIST_JSON_URL`); `NODE_POSTURE=mirror` defaults it to
  `https://config.decentraland.org/denylist.json`. Unset, nobody is
  deny-listed; upstream requires it.
- **Token claims.** The LiveKit token's `nbf` is the mint time minus the
  allowed clock skew (upstream `0`), and it carries the participant
  attribute `dcl.worldAuthorization`, the re-admission proof other
  participants can read.
- **Kicks on access change.** Owners and holders of any world or live-scene
  grant (deployment or streaming) are never kicked. A switch to an
  allow-list keeps participants already on the new list. A participant
  whose access cannot be decided stays.

- **`/about` health.** `healthy`, `acceptingUsers` and `content.healthy`
  are false only when this node's content sync has failed; a bootstrapping
  node or one with sync disabled reports the world healthy.
- **Realm name.** `WORLDS_REALM_NAME_STRIP_ENS` (default `true`) strips
  `.dcl.eth` from `/about`'s `realmName` for worlds published here, so a
  stock client does not resolve them against Decentraland's registry.
  Mirrored worlds keep the suffix.
- **Owner admission.** The owner is the name authority's current holder,
  memoized for 30 seconds per DCL NAME (60 seconds per ENS name; upstream
  one minute). It is asked only when the world's access setting refuses,
  admission waits on it for at most 3 seconds, and its answer (a fault
  included) is shared for 5 seconds. When the authority cannot answer,
  nobody is admitted as owner: the stored owner's refusal answers 503
  (upstream fails the whole mint), anyone else gets the access setting's
  401/403. The scene host's re-admission (periodic and on access changes)
  keeps a connected client whose answer is undetermined: an authority
  fault for the stored owner or a client admitted as owner, and a database
  fault reading the world, its grants or bans; a client admitted by the
  shared secret keeps that grant until the secret or access type changes.
  `GET /world/{name}/permissions` reports the live holder and, like
  `/limits`, answers 503 while the authority cannot answer; the comms owner
  read for stream-key notices is unavailable then too. Neither falls back
  to the stored owner. The owner-gated permission edits answer the same
  503 (upstream: 500). Publishes of one world are serialized, so a publish
  by a new holder always sees the owner it replaces.
- **Capacity check fails closed.** When the live occupancy of a world cannot
  be recovered, the comms mint answers 503 instead of counting zero
  participants and admitting the user.
- **Presence from LiveKit.** `/live-data` and
  `/wallet/{wallet}/connected-world` read the occupancy recovered from
  LiveKit every 30 seconds (about 5 seconds after a webhook), the same on
  every replica. `lastUpdated` is the last complete observation
  (`1970-01-01T00:00:00.000Z` before the first), `perWorld` is in name
  order, the `authoritative-server` participant is not counted, and after
  90 seconds without an observation both answer empty. A wallet's world
  counts scene rooms and ENS worlds; in several worlds it is the one it
  joined last, and a tie answers 503. A LiveKit room this node does not
  resolve (no catalogue row, an upstream-format or non-canonical name) is
  left out; while one could belong to a world, that world's mints answer
  503.
- **Scene room names.** A scene room is `w1:<base36 byte length of the world
  key>:<world key><scene id>`, with the scene id's case preserved. The
  prefix is fixed; there is no `SCENE_ROOM_PREFIX` or `COMMS_ROOM_PREFIX`
  setting. The world lobby room keeps `world-<name>`.
- **Room events.** Participant joins and leaves are published on the NATS
  subject `comms.room_events.v1`. No `peer.<id>.world.join|leave` subjects
  are published.
- **LiveKit webhook.** `POST /livekit-webhook` (also under
  `/worlds-content-server` and `/comms-gatekeeper`) is the gatekeeper's one
  webhook: a missing or bad signature answers 401, an event it cannot read
  (not JSON, no `id` or `createdAt`, missing participant fields) 400, and
  every accepted event 200 with an empty body. Upstream answers 400 without
  `Authorization`, 500 on a bad signature and echoes the body.
- **Offline comms when the SFU is down.** `/about` advertises
  `fixed-adapter:offline:offline` while LiveKit has no credentials or the
  realm's SFU liveness probe fails, instead of always advertising the
  signed-login adapter.
- **LiveKit only.** The comms adapter is always LiveKit; there is no
  ws-room mode and no `COMMS_ADAPTER` / `COMMS_FIXED_ADAPTER` setting.
- **Ban, deny-list and capacity on `/get-comms-adapter`.** The adapter route
  applies the same platform-ban, deny-list and capacity checks as the world
  comms mint.

## Storage and deployments

- **No `<entityId>.auth` blob.** A deployment's auth chain is stored only on
  its scene row and served in `GET /world/{name}/scenes`; there is no
  `/contents/<entityId>.auth` file.
- **One shared blob store.** World files live in the catalyst's single
  hash-addressed store, so a deployment may reference any blob already in
  it (including Genesis content) without uploading it again. Upstream could
  only reuse files from its own bucket.
- **Thumbnails** are hashed as CIDv1.
- **ENS names.** With `ALLOW_ENS_DOMAINS=true` the holder is read over
  `ETH_RPC_URL` (required then; the process refuses to start without it).
  Only names whose labels the upstream's UTS46 normalization leaves
  unchanged resolve: lowercase ASCII letters, digits and inner `-`, with no
  `--` at the third and fourth characters. Upstream decodes `xn--` labels
  and resolves other Unicode names; here those have no owner. A wrapped
  name has an owner only while the ENS Registry still gives its node to
  the NameWrapper (upstream trusts the NameWrapper alone).
- **Owner reconcile.** `worlds.owner` is rewritten from the name authority
  every `WORLDS_OWNER_RECONCILE_INTERVAL_SECS` (default 12 hours, as
  upstream) for worlds with a locally published scene, ENS worlds
  included. A previous holder the world's access setting no longer admits
  is removed from the world's live rooms, as it is when a new holder's
  publish replaces the stored owner.
- **Unowned names.** Bare names, and ENS names while `ALLOW_ENS_DOMAINS`
  is off, have no owner, so in comms nobody gets the owner's implicit
  scene-admin and streaming rights over them (explicit grants still
  apply), and their limits route answers 404.
- **Newer-entity check** compares against the lower of the stored entity's
  timestamp and its row's update time, so a future-dated mirrored or
  imported entity cannot block redeploys.
- **Validation errors** report the first failure only, and scene schema
  errors name the failing path.
- **Scene pointers** are capped at 90,601 (every in-bounds parcel once); a
  longer list fails with `must NOT have more than 90601 items`, which
  upstream does not have.
- **Grantee publishes** resolve the world's NAME holder live. If the names
  authority cannot answer, the publish is refused with 503.
- **Publish world names.** `POST /entities` takes a `.dcl.eth` world (and an
  `.eth` one with `ALLOW_ENS_DOMAINS`); any other name answers 400
  `Deployment failed: Only .dcl.eth world names are supported for
  publishing (got "<name>")` (`.dcl.eth and .eth` with ENS on), checked
  right after the scene schema. Upstream refuses such a name at the
  permission check. `<address>.dcl.eth` is that address's personal world,
  which the address may publish without holding a NAME.
- **Authority faults on writes fail closed.** When the name authority
  cannot answer, a publish answers 403 `Not authorized: could not verify
  deployment permissions (deploy denied)` and the world undeploy its
  authority status (503 for a store fault). A grantee publish whose NAME
  has no owner answers 400 `Cannot deploy scene "<id>" to world "<name>":
  owner address could not be resolved.`, and a storage allowance that
  cannot be read answers 503 `Deployment failed: the storage allowance
  could not be determined right now`. Upstream answers 500 for these (400
  `Could not determine owner for world <name>` while sizing). A deploy 500
  is `{"error": "Internal Server Error"}`.
- **Publish auth chains.** A contract-wallet (EIP-1654) chain is verified
  over `ETH_RPC_URL` when it is set, and answers 503 `The auth chain could
  not be verified right now: …` when the RPC cannot answer; upstream
  validates without a provider and refuses every such chain. Refusals use
  `@dcl/crypto` 3.7.0's texts for a malformed chain, a signer mismatch, a
  missing signature, an expired key and the final authority (an expired key
  is named by the chain's first ephemeral link); an unrecoverable signature
  and the shared verifier's structure rules (a signed-entity link before the
  last) answer the verifier's own text, and of the `AuthChain` schema
  errors only an unknown link type has upstream's text.
- **Publish forms.** `POST /entities` reads only `multipart/form-data`
  (upstream's parser also reads `application/x-www-form-urlencoded`); a
  body cut short answers busboy's `Unexpected end of form`, any other
  unreadable body the parser's own text. A part name longer than 200 bytes
  answers 400 `Form field name longer than 200 bytes` (upstream's busboy
  applies no name limit to multipart parts, only 16 KiB to a part's
  headers). Its
  uploads take a slot of `MAX_CONCURRENT_UPLOADS` (refused with upstream's
  byte-budget 503, `Server is buffering too many uploads, please retry
  shortly.`) and count files against `MAX_IN_FLIGHT_UPLOAD_FILES` instead of
  upstream's 16 KiB charge per file. The gateway reads the whole body
  before the app sees it, so a partial upload's first batch is dated when
  its body has reached the app. The gateway also limits a client address to
  5 deploys a second (burst 4) and 10 uploads in flight, which upstream does
  not; past them it answers 429 with the per-source upload refusal and
  `Retry-After: 5`, and a body over 360 MiB gets the app's 413 JSON.
- **No deployment events.** Publishes and undeploys publish no SNS
  `WorldDeploymentEvent`, `WORLD_SETTINGS_CHANGED`,
  `WorldUndeploymentEvent` or `WorldScenesUndeploymentEvent`. Readers in this
  tree (explore's places, the settings reads) read the worlds tables the
  write commits; a publish also updates the scene identities and queues the
  forward outbox in its transaction, and every publish and undeploy
  re-checks the owner's storage block.
- **Per-source upload limits** key on the gateway's `X-Real-IP` (IPv6 by
  /64), not `cf-connecting-ip`. Deploys and settings uploads share one
  per-source budget (`MAX_CONCURRENT_UPLOADS_PER_SOURCE`,
  `MAX_IN_FLIGHT_UPLOAD_BYTES_PER_SOURCE`, the latter defaulting to one
  maximum-size upload).
- **Per-world limits** come from `MAX_PARCELS`, `MAX_FILE_COUNT` and the
  `worlds.world_limit_overrides` table; there is no `WHITELIST_URL`. Only a
  row with a size takes the world out of its owner's storage allowance; a
  row that sets parcels alone leaves the allowance in force. As upstream, an
  ENS world's size cap is `ENS_MAX_SIZE` whatever its override row says.
- **Retired scenes.** Replaced and undeployed scenes are kept for
  `SCENE_EVICTION_TTL_MS` and removed by an hourly sweep. Scenes a mirror
  pass retires are deleted at once.
- **`/gc`** reports `removedBytes`. A dry run reclaims no expired partial
  upload and reports `expiredUploads: null`. Every `/admin/*` route answers
  401 for a missing or wrong token. The bearer is
  `CATALYRST_WORLDS_ADMIN_TOKEN` (no `AUTH_SECRET`; unset refuses every
  request), matched against everything after `Bearer `. Like every worlds
  write it answers 503 while `WORLDS_WRITES_ENABLED` is off. It reads
  `dry_run`, `min_age_secs` and `allow_empty_active_set` (an unparsable
  value answers 400), refuses an empty reference set without
  `allow_empty_active_set=true`, and answers 503 `A garbage collection run
  is already in progress.` to a second concurrent run.
- **Partial uploads** follow the upstream contract and limits, with these
  differences:
  - A stored file is free of staging charges when any domain of the shared
    blob store names it, not only world scenes and thumbnails.
  - The collector keeps every file a pending upload names, expired uploads
    included, until the cleanup removes the upload. The cleanup unlinks
    only files the upload's own batches stored, nothing else names and are
    older than the collector's age floor; it leaves any other file to the
    collector.
  - A file found stored when the upload began and gone by its completion
    is listed in `missing` again (202) instead of failing the publish.
  - Batches of one entity are serialized within a process by an in-memory
    lock; across replicas each staging step is atomic in the database.
    There is no `CONTENT_LOCK_CONNECTIONS`. `MAX_CONCURRENT_UPLOADS` is
    lowered to three quarters of `PG_POOL_SIZE` (at least one) with a
    startup warning when it is set higher. Single-request and partial
    publishes share that limit, so the lowered value bounds both.
  - A personal world accepts partial uploads only once it is published on
    this realm; its first scene goes in a single request.
  - `PENDING_DEPLOYMENT_TTL`, `MAX_PENDING_DEPLOYMENTS_PER_DEPLOYER`,
    `MAX_PENDING_BYTES_PER_DEPLOYER`, `MAX_PENDING_BYTES` and
    `MAX_PARTIAL_UPLOAD_BYTES_PER_MINUTE` are the variables the content
    server's partial uploads read, with the same defaults. Each value
    applies to worlds and content separately: world uploads and content
    uploads keep their own counts, bytes and rate windows.
  - `MAX_PENDING_BYTES_PER_SOURCE` (default 4 GiB, at least
    `MAX_PENDING_BYTES_PER_DEPLOYER`) caps the bytes staged by the live
    uploads one client address started (429 with `Retry-After`).
  - The cleanup runs as a fleet-wide leased job. Its `Retry-After` estimate
    uses the last run this process finished.
  - Partial batches record only the `total` processing stage, labelled
    `mode="partial"`.
- **`GET /worlds` search** adds trigram similarity only when the `pg_trgm`
  extension is installed. `%` and `_` in the query match literally.

## Reads, mounts and admin

- **Mounts.** Worlds clients use the `worlds-content-server.<domain>` host,
  which the gateway maps onto the `/worlds-content-server` prefix, or the
  prefix itself. At the node root, `GET /contents/{hash}`,
  `GET /available-content`, `POST /entities`, `POST /entities/active` and
  `GET /status` are the catalyst content server's; `DELETE
  /entities/{name}`, `/ipfs/{hash}` and `/gc` answer only under the prefix,
  and the worlds status is also at `/worlds/status`. `/about` and `/index`
  link files under `<PUBLIC_URL>/worlds-content-server/contents/`.
- **CORS.** The worlds routes answer a preflight with 204 and reflect
  the request's `Origin` with `Access-Control-Allow-Credentials: true` and
  `Vary: Origin`, allows `PATCH` as well, exposes `*` when the handler
  names no header, and sets `X-Content-Type-Options: nosniff`. Upstream
  answers `Access-Control-Allow-Origin: *`.
- **`/available-content`** answers a cid on the content denylist as
  `available: false`.
- **`POST /entities/active`** answers a body that is not JSON with 400
  `Invalid request. Request body is not valid` (upstream: 500) and caps the
  body at 256 KiB (413).
- **`/index`** leaves out worlds an operator disabled. It pages over worlds
  with a deployed scene, so a page holds `limit` worlds while more remain. A
  page is reused for 30 seconds (or until a mirror pass changes the scenes);
  `lastUpdated` is its build time.
- **`/status`** counts rooms and users from this node's presence, live
  (upstream: LiveKit's room listing, cached 60 seconds), answers
  `adapterType: "offline"` without `statusUrl` when no LiveKit host is set,
  and adds `personalWorlds` and `mirror`.

## Notifications

- **Delivery.** World notices are written to the catalyst's own inbox and
  email outbox, in the same transaction as the grant edit or quota change,
  instead of being published to SNS. Re-detecting the same quota block keeps
  the first notice unchanged; a new block notifies again even on the same
  day. Every grant or revoke is its own notice, so the latest notice matches
  the current grant. One edit notifies at most 50 added and 50 removed
  addresses. The notice description does not name the world (it is in the
  `world` field).
- **Last parcel removed.** Removing a parcel-scoped grant's last parcel
  sends `worlds_permission_revoked`, because the grant is deleted.
- **Settings events.** `PUT /world/{name}/settings` publishes no
  `WorldSettingsChangedEvent` or `WorldSpawnCoordinateSetEvent`; the
  explore crate reads the worlds tables directly.
- **No settings events.** An access change is not published as a
  `world-settings-changed` event; in-tree readers read the stored access
  type.

## Permissions

- **Community ids** in an allow-list must be UUIDs.
- **Allow-list caps.** `ACCESS_MAX_WALLETS` (1..=10,000, default 1000) and
  `ACCESS_MAX_COMMUNITIES` (1..=1,000, default 50) are refused at startup
  outside their range; upstream takes any number, and `0` there refuses
  every addition.
- **Allow-list documents** are stored normalized: `GET .../permissions`
  returns the access wallets lowercased and deduplicated, in the order they
  were added, and the community ids as lowercase UUIDs, whatever spelling
  and order were posted.
- **Permission payloads** are checked more strictly: `deployment`,
  `streaming` and access wallets must be addresses (400 `Invalid address:
  <w>.`), `nft` must be a string, a `secret` that is not a non-empty string
  is a 400 (upstream: 500 for a non-string), and a parcel coordinate outside
  32 bits is 400 `Invalid parcels: …` (upstream stores it).
- **Parcel holder lookups** (`POST .../permissions/{permission}/parcels`)
  match the requested parcels in canonical form, so `00,01` finds a grant on
  `0,1`.
- **Removing a grant's last parcel** deletes the grant. Upstream keeps the
  row, which then becomes world-wide.
- **Streaming permission** is enforced when a participant asks to stream,
  not only recorded.
- **Shared-secret hashes** use bcrypt cost 12 (upstream: 10); existing
  hashes verify unchanged.

## World metadata and settings

- **Writes switch.** Every worlds write (publishes, undeploys, settings and
  permission edits, the operator and storage routes) answers 503
  `{"ok": false, "error": "Service Unavailable", "message": …}` with
  `Retry-After: 3600` while `WORLDS_WRITES_ENABLED` is off, which is the
  default, and while the restore stamps do not vouch for this node
  (DATABASE.md, "Bootstrap, schema versions and local writes"). Reads,
  `POST /entities/active`, the parcel-holder lookup, the world-storage
  export (`POST /world-storage/exports`) and the comms mints stay open. A node replacing the upstream service sets it once a restore of its
  database, blobs and credentials has been shown to work.
- **`/about` configuration.** `GLOBAL_SCENES_URN` is split on runs of
  whitespace, so it never yields an empty URN, and an `ETH_NETWORK` other
  than `sepolia` reports `networkId: 1` (upstream answers 500 for a network
  it does not know). A visible minimap without its own images links this
  node's `/api/v1/minimap.png` and `/api/v1/estatemap.png` unless
  `MAP_PARCEL_VIEW_URL` / `MAP_ESTATE_VIEW_URL` name others; upstream links
  `https://api.decentraland.org/v1/minimap.png` and `/v1/estatemap.png`.
- **Settings superset.** `GET`/`PUT /world/{name}/settings` also carry
  `realm_name_override` and `preview_wearable_urns` (always present, `null`
  or a list), and the PUT form accepts both; `preview_wearable_urns` may
  repeat, like `categories`.
- **Settings uploads.** `spawn_coordinates` must be two integers (spaces
  around them allowed); upstream's `parseInt` also takes `5abc,3` or
  `5.9,3` and stores them as sent. The form must be `multipart/form-data`
  (upstream's parser also reads `application/x-www-form-urlencoded`). The
  upload takes a slot of `MAX_CONCURRENT_UPLOADS` (503 `Server is handling
  too many concurrent uploads, please retry shortly.` when none is free)
  and its processing runs under `DEPLOYMENT_PROCESSING_TIMEOUT_MS` (408
  `Deployment processing exceeded the <n>ms deadline.`); upstream has
  neither on this route. The per-source admission runs after the signed
  fetch (upstream: before it).
- **Scene queries.** `POST /world/{name}/scenes` checks the Content-Type
  before it reads the body and reads at most 2 MiB; a larger body answers
  413 `Payload Too Large` (plain text, `Connection: close`, as
  `@dcl/http-server` answers a body over its `maxBodySize`). Upstream sets
  no body limit on this route.
- **Deny list in world listings (shared with upstream).** `GET /worlds`
  filters the deny list as upstream's `getBannedNames` does: each entry
  loses its first `.eth` and `.dcl` and is compared case-sensitively with
  the stored name, so only a bare name equal to a stripped entry is hidden.
  A deny-listed `<name>.dcl.eth` stays listed, with its title, description
  and thumbnail, while `/about` and the manifest answer 404 for it. This is
  a moderation gap both services have, kept for parity.
- **World listings.** In `GET /worlds`, `blocked_since` is set only for a
  storage-quota block of a DCL NAME or ENS world published here (upstream:
  any block of the stored owner), and `owner` is `null` for a world with no
  owner (bare names, mirrored worlds); `GET /wallet/contribute` answers the
  same `owner`.

## World storage

`/world-storage` follows
[`decentraland/world-storage-service`](https://github.com/decentraland/world-storage-service)
(compared at `d55f24c`).

### Authorization

- **Deployers.** A world's `deployment` grantee reaches a scene's storage
  only when the grant covers that scene's whole footprint (a world-wide
  grant covers every scene). Upstream admits every wallet in the world's
  deployment allow-list, parcel-scoped grantees included, to every scene.
- **Genesis collaborators.** A wallet in a Genesis scene's
  `logsPermissions` is admitted only while the LAND index has complete,
  fresh ownership history for the whole footprint and no parcel changed
  owner after the scene was deployed. Upstream admits it on the active
  deployment alone.

### HTTP

- **Deeply nested values.** A value nested deeper than about 4,500 levels
  is accepted and stored, up to the depth Postgres' `jsonb` input accepts
  (then 500 `{"error": "Internal Server Error"}`). Upstream answers 500 from
  that depth on, where ajv's recursive validation and `JSON.stringify`
  exhaust Node's stack.
- **Schema errors of deep values.** When the non-finite number that fails
  the value's `anyOf` sits more than 128 containers deep, `data` holds only
  the outermost `anyOf` error at `/value`. Upstream lists every level's
  errors, a body that grows with the square of the depth (6.7 MB at 1,000
  levels).
- **NUL in a query.** A `%00` in the query string (a list's `prefix`)
  answers 400 `{"error": "request path or query contains a NUL byte"}`
  before the signed fetch is checked. Upstream passes it to Postgres and
  answers 500. A NUL in a key answers upstream's 400 after authorization.
- **`GET /collaborator` lists local scenes only.** Mirrored world scenes
  are left out, as their `logsPermissions` grant no storage access on this
  node; upstream lists every scene its deployment consumer indexed.

### Quotas, export and configuration

- **Quotas of retired scenes.** A world's world, player and env quotas
  count its active scenes and legacy scopes but not scenes retired by an
  undeploy or by a redeploy into a new place (an inactive
  `explore.world_scene_place` identity); the `/usage` routes report the
  same numbers. Upstream counts every stored row of the world forever.
  The retired scenes' rows are kept as a safety copy: no undeploy,
  redeploy, world deletion, sweep or mirror deletes them, and the clear
  routes reach only the active scene the request resolves to.
- **Collaborators.** No deployment consumer (upstream's SQS queue of
  world and catalyst deployments) maintains a collaborator table:
  `GET /collaborator` and the collaborator grant read the active
  deployments' `metadata.logsPermissions` (`worlds.logs_permissions_decoded`) on
  every request.
- **Exports.** Both row streams read one read-only repeatable-read
  snapshot through a pooled connection; upstream reads the world rows and
  the player rows as separate statements. Statuses, cooldown, timer,
  caps, logging and the gzip stream otherwise follow upstream.
- **Metrics.** The worlds routes record `http_requests_total`,
  `http_request_duration_seconds` and `http_request_size_bytes` by
  `method`, `handler` and `code`, as upstream's defaults; `handler` is the
  matched route as this node mounts it (for example
  `/world-storage/values/{key}`, upstream `/values/:key`), shared with the
  node's other routes. The logger and pg metric families are not exported.
- **Cache staleness (shared with upstream).** The single-value LRU is per
  process; with several storage-serving processes, a read may be up to
  `STORAGE_CACHE_TTL_SECONDS` stale after another process's write.
- **Encryption key.** `ENCRYPTION_KEY` is required, as upstream. This node
  has no committed development key, so upstream's production refusal of
  that key has no counterpart.
