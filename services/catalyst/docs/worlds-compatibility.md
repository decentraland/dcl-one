# Worlds: compatibility differences

The `worlds` crate serves the worlds content server's HTTP surface (under
`/worlds-content-server`, and at the root where no catalyst route has the
path; see "Mounts") and its comms admission. The wire
contract follows
[`decentraland/worlds-content-server`](https://github.com/decentraland/worlds-content-server).
This page records intended differences and their operational limitations.
A difference not listed here is a gap, not a decision.

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
- **Allowance monitoring.** Before enabling production Worlds writes,
  verify that every serving node has effective `ETH_RPC_URL` and
  `RPC_ENDPOINT_POLYGON` values for the intended chains. Mirror nodes and
  nodes with no posture default to Decentraland's public RPCs. Scrape each
  node's `SERVE_METRICS_BIND` endpoint in production. The
  `worlds_allowance_read_failures_total` counter increments once per
  terminal chain lookup failure.
- **Legacy content keys.** The worlds `/contents/{key}` route also accepts a
  64-hex sha256 key and serves the blob whose raw-leaf CIDv1 has that
  digest, which covers files of one chunk (256 KiB) or less. The mirror and
  cutover importer record verified SHA-256 aliases for thumbnails larger
  than one chunk and up to 1 MiB. The route also serves CIDv0 (`Qm…`) keys.
  Invalid and missing keys answer 404 without a body. A key on the content
  denylist answers 404. With `READ_THROUGH_ENABLED`, the route tries to fetch
  a missing key from the sync sources before answering 404.
- **Content types.** The worlds `/contents` and `/ipfs` routes answer
  `file-type@16`'s Content-Type for scene formats: images, glTF binary,
  audio, video, fonts, PDF, WebAssembly, gzip and ZIP. Other formats, and a
  ZIP's office or EPUB subtypes, are `application/octet-stream` (or the type
  of a ported signature their bytes also match) or `application/zip`. A
  document type (`text/*`, `*/xml`, `*+xml` or HTML; of the ported checks,
  XML) is `application/octet-stream` (upstream: `application/xml`): these
  routes share the node's origin, where a browser would run an XHTML or SVG
  document's scripts. Every answer carries `Content-Security-Policy: sandbox`
  and `X-Content-Type-Options: nosniff`, so a file opened as a page runs
  sandboxed. A file that opens with `{` or `[` is
  `application/json` (upstream: `application/octet-stream`). A file of more
  than 16 chained ID3 tags, 256 PNG chunks before the image data, or 64 EBML
  header elements is `application/octet-stream`.
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
  (upstream also clears it when the owner joins). A caller already out of
  attempts gets 429 before any ban or access check, as upstream; that check
  charges nothing. It runs only on a world the mint admits: an unknown,
  blocked or name-deny-listed world answers 404 first. Upstream's
  `/worlds/{name}/comms` and scene mints also run it for a blocked world and
  for every world the DAO name list refuses (`<label>.dcl.eth` and
  `<label>.eth`, whatever the entry's case): the access read leaves out only
  names equal to a `BLOCKED_WORLD_NAMES` entry or to a list entry without its
  suffix, and no stored world name equals the latter. So a caller throttled on
  such a shared-secret world gets 429 there. If the counter
  cannot be read or written the mint answers 503.
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
- **Kicks on access change.** Owners and wallets with deployment or streaming
  grants keep their place in an open world room when an access edit would
  otherwise remove them. A parcel-scoped grant must cover an active scene
  in the room inventory. Platform and scene bans still apply. A switch to
  an allow-list keeps participants already on the new list. A participant
  whose access cannot be decided stays.

- **`/about` health.** `healthy`, `acceptingUsers` and `content.healthy`
  are false only when this node's content sync has failed; a bootstrapping
  node or one with sync disabled reports the world healthy.
- **Realm name.** `WORLDS_REALM_NAME_STRIP_ENS` (default `true`) strips
  `.dcl.eth` from `/about`'s `realmName` for worlds published here, so a
  stock client does not resolve them against Decentraland's registry.
  Mirrored worlds keep the suffix. The comms mints (`/worlds/<name>/comms`,
  `/get-comms-adapter/world-<name>`) take the stripped name for the
  `.dcl.eth` world when no world is stored under the bare label.
  `/get-scene-adapter` takes it for that world's active scene when the
  signed `sceneId` equals the signed `realmName` (the Unity explorer signs
  a realm without a scene manifest this way, and no realm URL) and the
  active scene was published here. `REALM_NAME`, the preview realm names
  and a realm name signed beside a scene id keep their shared room.
  A scene's signed request with the stripped name stands in that world when
  the signed realm host is the realm URL (`<host>/world/<name>.dcl.eth`, the
  web Explorer) or `worlds-content-server.<domain>` alone, with or without
  a scheme and a trailing `/` (the Unity client); this node's `REALM_NAME`
  signed there keeps its shared reading.
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
  LiveKit every 30 seconds. A webhook can bring the next observation on
  its receiving replica forward to about 5 seconds. Both endpoints answer
  503 with `Retry-After` before the first complete observation and after
  90 seconds without one. A fresh observation can report zero occupants
  or a 404 for a disconnected wallet. `/live-data` sets `lastUpdated` to
  the latest complete observation and reports lobby counts in world-name
  order and excludes the `authoritative-server` participant.
  Connected-world includes scene rooms and ENS worlds. It reports the latest
  world the wallet joined and picks the first normalized name in alphabetical
  order for a tie. The public projection accepts legacy scene room names
  when they match exactly one current catalogue entry. Admission and social
  presence continue to use canonical room names.
- **Scene room names.** A scene room is `w1:<base36 byte length of the world
  key>:<world key><scene id>`, with the scene id's case preserved. The
  prefix is fixed; there is no `SCENE_ROOM_PREFIX` or `COMMS_ROOM_PREFIX`
  setting. The world lobby room keeps `world-<name>`.
- **Room events.** Participant joins and leaves are published on the NATS
  subject `comms.room_events.v1`; every `serve` applies World joins and leaves to
  social online status, which treats being inside a World as online. No
  `peer.<id>.world.join|leave` subjects are published.
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
- **`place_id` on `GET /scene-admin`.** The signed scene names the place for
  a scene and for the authoritative server, as upstream. A `place_id` query
  (upstream reads none) must name that scene, by its entity id or by the key
  of its place; another value answers 400. A signed scene that is not
  deployed answers 400 and a resolver fault 503, never an empty list.
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
- **Thumbnails** are hashed as CIDv1. A mirrored world's thumbnail is
  copied from its source's `/contents` and stored under the CIDv1 of its
  bytes, so for a thumbnail uploaded to the source (stored there under the
  SHA-256 hex of its bytes) `GET /world/{name}/settings` names the CID where
  the source names the hex key. For larger thumbnails, the next successful
  mirror of those settings or a replay of the cutover import records the
  alias for the saved hex URL. The alias keeps the blob reachable after
  settings edits. A failed source read keeps the existing thumbnail. A
  successful read of a blob with an unsupported format leaves the world
  without a thumbnail.
- **ENS names.** With `ALLOW_ENS_DOMAINS=true` the holder is read over
  `ETH_RPC_URL` (required then; the process refuses to start without it).
  Only names whose labels the upstream's UTS46 normalization leaves
  unchanged resolve: lowercase ASCII letters, digits and inner `-`, with no
  `--` at the third and fourth characters. Upstream decodes `xn--` labels
  and resolves other Unicode names; here those have no owner. A wrapped
  name has an owner only while the ENS Registry still gives its node to
  the NameWrapper (upstream trusts the NameWrapper alone).
- **DCL NAME holder reader.** A DCL NAME world's owner is the NAME's
  current holder, read from the nft index on a node that walks it
  (`MARKETPLACE_NFT_INDEX_ENABLED` on mainnet), which authorizes only while
  every block it walked is folded and published (between walking a
  transfer and publishing it, and while the index is behind, owner writes
  and admission answer 503); elsewhere from `MARKETPLACE_SUBGRAPH_URL`
  (upstream's marketplace subgraph), which has no default and only the
  `mirror` posture sets. Without either a DCL NAME world has no owner for writes: publishes,
  the owner-gated permission, settings, undeploy and storage routes, and
  the owner's scene-admin rights (admins, bans, stream key, Cast) are
  refused (upstream reads the stored owner first, which keeps naming a
  former holder until a publish replaces it). The reads still
  use the stored owner then: `GET /world/{name}/permissions`, `/limits`,
  owner admission, the ban-protected set and the quota pass. `catalyst serve`
  with `WORLDS_WRITES_ENABLED=true` refuses to start without either, before
  it opens the database (`catalyst index` takes no world writes and skips
  this rule), unless `IGNORE_BLOCKCHAIN_ACCESS_CHECKS=true` (a node
  without chain access, such as the `journey` posture), where it logs a
  warning and serves personal worlds.
- **Quota block on a reader fault.** Admission applies the quota block of
  the NAME's live holder when it differs from the stored owner. When the
  holder cannot be read (a reader fault or timeout), or without a reader,
  the stored owner's block applies, so admission stays available during a
  reader outage at the cost of a block that may trail a NAME transfer.
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
- **`/reprocess-ab`** takes the same bearer as `/gc` and answers the same
  validator, `No scenes found for reprocessing` and 200 bodies as upstream,
  but it hands the scenes to the local abgen (`ABGEN_ADMIN_URL`) instead of
  publishing `WorldDeploymentEvent`s to SNS. For each scene it asks abgen for
  the `windows` and `mac` manifests, and abgen converts a manifest it does not
  hold. A scene abgen already converted is not built again. `successful`
  counts the scenes abgen took on every platform, including one still
  converting when the 10 s hand-off times out; `failed` counts the ones it
  refused or could not be reached for. Without `ABGEN_ADMIN_URL` the route
  answers 500, as upstream does without `AWS_SNS_ARN`. abgen fetches the
  scene's files from its own content source, so it needs its worlds content
  URL (`ABGEN_WORLDS_CONTENT_URL`) set to this node's
  `<PUBLIC_URL>/worlds-content-server`. The route stays open while
  `WORLDS_WRITES_ENABLED` is off, since it writes nothing here.
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
  /entities/{name}`, `/ipfs/{hash}`, `/gc` and `/reprocess-ab` answer only
  under the prefix, and the worlds status is also at `/worlds/status`.
  `/about` and `/index` link files under
  `<PUBLIC_URL>/worlds-content-server/contents/`.
- **CORS.** The worlds routes answer a preflight with 204 and reflect
  the request's `Origin` with `Access-Control-Allow-Credentials: true` and
  `Vary: Origin`, allows `PATCH` as well, exposes `*` when the handler
  names no header, and sets `X-Content-Type-Options: nosniff`. Upstream
  answers `Access-Control-Allow-Origin: *`. `/world-storage` reflects any
  `Origin` the same way but never sends `Access-Control-Allow-Credentials`:
  signed fetch carries its auth in headers, not cookies. Upstream
  world-storage reflects only the origins that match its `CORS_ORIGINS`
  patterns and exposes `ETag, Retry-After`.
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
- **Name deny list after a failed read.** For five seconds after a failed
  read of the `banned-names` list, the last list that read applies with
  `BLOCKED_WORLD_NAMES` (the configured names alone if none has read since
  start); then the list is read again. Upstream keeps an empty list for an
  hour after a failed fetch.
- **Gateway rate limits.** Reads (GET, HEAD, OPTIONS) have no rate limit,
  as upstream. Writes, and the `admin`, `gc` and `reprocess-ab` routes, are
  limited to 30 requests a second per address (burst 60), answered with
  nginx's 429; settings uploads also take at most 10 connections per address.
  Upstream limits none of them.

## Notifications

- **Delivery.** World notices are written to the catalyst's own inbox and
  email outbox, in the same transaction as the grant edit or quota change,
  instead of being published to SNS. Quota notices deduplicate by wallet,
  notice kind and UTC date of block creation. Warning, restriction and
  restoration are separate kinds; restoration uses the block's creation day,
  even when access is restored on a later day. Repeats keep the first row's
  metadata, occurrence time, read state and email delivery state without
  requeuing email. Every grant or revoke is its own notice, so the latest
  notice matches the current grant.
  One edit notifies every added and removed address in
  batches of at most 50, all in the edit transaction. The description names
  the world when it matches the world-name pattern, or says "a world"
  otherwise; the `world` field keeps the world name.
- **Quota history at cutover.** When the block's exact creation time is
  available, the writer links existing native notices for all three quota
  kinds to their daily keys on any quota emission, retaining their row IDs,
  contents, read state and email delivery records. Already closed native
  blocks and imported history may lack the information needed for that link.
  Such unlinked history can coexist with one additional daily notice per
  wallet and kind at cutover; this change does not consolidate all historical
  duplicates. Later repeats use the daily key. Retire quota writers that do
  not set daily keys before relying on daily uniqueness. The
  [quota identity decision](../../../docs/adr/2026-11-21-world-quota-notice-identity.md)
  states this boundary. Quota enforcement, grace timing and the notification
  deadline metadata are unchanged.
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
- **Deployment and streaming lists.** When supplied, `wallets` must be an
  array of wallet address strings. Invalid values answer 400 and leave
  existing grants in place.
- **Allow-list caps.** `ACCESS_MAX_WALLETS` (1..=10,000, default 1000) and
  `ACCESS_MAX_COMMUNITIES` (1..=1,000, default 50) are refused at startup
  outside their range; upstream takes any number, and `0` there refuses
  every addition. Deployment and streaming allow-lists have no application
  count cap, as upstream. Auth-chain validation limits the complete signed
  payload to 100,000 bytes (`dcl-auth`'s `MAX_AUTH_LINK_FIELD_LEN`); HTTP
  header limits also apply to the signed metadata. These byte limits do not
  define a fixed wallet count. Notification batches contain at most 50
  addresses, but the total edit transaction grows with the number of changed
  addresses.
- **Allow-list documents** are stored normalized: `GET .../permissions`
  returns the access wallets lowercased and deduplicated, in the order they
  were added, and the community ids as lowercase UUIDs, whatever spelling
  and order were posted.
- **Permission payloads** are checked more strictly: `deployment`,
  `streaming` and access wallets must be addresses (400 `Invalid address:
  <w>.`), `nft` must be a string, a `secret` that is not a non-empty string
  is a 400 (upstream: 500 for a non-string), and a parcel coordinate outside
  32 bits is 400 `Invalid parcels: …` (upstream stores it).
- **Deployment and streaming allow-list edits** clear the grants when
  `wallets` is omitted, `null` or an empty array. Other non-array values
  and non-string elements answer 400 and preserve the grants.
- **Parcel holder lookups** (`POST .../permissions/{permission}/parcels`)
  match the requested parcels in canonical form, so `00,01` finds a grant on
  `0,1`.
- **Removing a grant's last parcel** deletes the grant. Upstream keeps the
  row, which then becomes world-wide.
- **Streaming permission** is enforced when a participant asks to stream,
  not only recorded.
- **Shared-secret hashes** use bcrypt cost 12 (upstream: 10); existing
  hashes verify unchanged.
- **Mirrored worlds** take the source's access type and allow-list wallets
  and apply this node's permission model to them. Deployment and streaming
  grants are not copied, so their lists and `summary` are empty; a
  shared-secret world refuses every secret, because the source does not
  publish its hash; allow-list communities are matched against this node's
  members. Once the owner changes the access here, the mirror keeps that
  setting while they hold the name. A world's own grants, secrets and
  members come with a data migration, not through the mirror.

## World metadata and settings

- **Writes switch.** Every worlds write (publishes, undeploys, settings and
  permission edits, the operator and storage routes) answers 503
  `{"ok": false, "error": "Service Unavailable", "message": …}` with
  `Retry-After: 3600` while `WORLDS_WRITES_ENABLED` is off, which is the
  default (DATABASE.md, "Bootstrap, schema versions and local writes"). Reads,
  `POST /entities/active`, the parcel-holder lookup, the world-storage
  export (`POST /world-storage/exports`) and the comms mints stay open. A node replacing the upstream service sets it once its database, blobs
  and credentials are backed up and a restore has been tested.
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
  repeat, like `categories`. A `realm_name_override` that names another
  world with a scene on this node answers 400: its label (read as
  `<label>.dcl.eth` beside the worlds host) or its full `.eth` name, in any
  case. The gatekeeper reads a signed realm name as the world it names, so
  the override would seat this world's visitors in that world's room. Any
  other override is accepted, and so is the world's own name. `/about` makes
  the same check each time it answers: an override whose world was deployed
  here after it was saved is not advertised, and `realmName` falls back to
  the world's own name.
- **Mirrored spawn.** A scheduled mirror refresh reads valid `spawn_coordinates`
  from the source's world settings even when the scene set is unchanged.
  It stores a valid spawn as `x,y` with each axis from -150 to 150.
  A missing or invalid setting leaves the spawn in place when the scene set
  is unchanged. A full sync falls back to `/about` and can use the scene base
  when a world has no stored spawn. A valid source spawn replaces a spawn set
  here on the next refresh, independently of owner overrides for other
  mirrored settings. A spawn-only refresh leaves `settings_version` unchanged.
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
  and `/index` filter the deny list as upstream's `getBannedNames` does:
  each `BLOCKED_WORLD_NAMES` entry is a whole lowercased world name and
  hides that world; each DAO list entry loses its first `.eth` and `.dcl`
  and is compared case-sensitively with the stored name, so only a bare
  name equal to a stripped entry is hidden. A `<name>.dcl.eth` on the DAO
  list stays listed, with its title, description and thumbnail, while
  `/about`, the manifest and the comms mints answer 404 for it. This is a
  moderation gap both services have, kept for parity; `BLOCKED_WORLD_NAMES`
  is the way to hide a full name.
- **World listings.** In `GET /worlds`, `blocked_since` is set only for a
  storage-quota block of a DCL NAME or ENS world published here (upstream:
  any block of the stored owner), and `owner` is `null` for a world with no
  owner (bare names, mirrored worlds); `GET /wallet/contribute` answers the
  same `owner`.
- **Places freshness.** A committed deploy, undeploy, settings edit,
  spawn edit, access change, operator block change, or owner reconciliation
  queues the affected world for a Places listing refresh. `places-derive`
  checks for pending worlds every two seconds and refreshes up to 64 per
  pass. The job still reconciles the full catalogue
  every `PLACES_DERIVE_INTERVAL_SECS` (300 seconds by default). A full pass
  already in progress or a backlog can delay the targeted refresh.

## Scene Insights cutover

Set `WORLDS_CONTENT_SERVER_URL` in the Scene Insights deployment to the
public dcl-one base with the `/worlds-content-server` prefix when Worlds
becomes authoritative. Verify that the base serves
`GET /world/{name}/permissions` and `GET /world/{name}/scenes` for a World
published on dcl-one. Then use signed Scene Insights requests to check a
wallet listed in the active scene's `metadata.logsPermissions` and a separate
wallet granted deployment permission for that scene. Each check should use
a wallet whose access depends on the permission being tested.

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
  owner after the scene was deployed (see "Genesis access"). Upstream
  admits it on the active deployment alone.

### Genesis access

A Genesis request names one parcel in its signed metadata. Storage
resolves the active scene standing there and admits the signer by its role
at that parcel. Scene admins (`social.scene_admins`) get no Genesis storage
access.

- **Roles.** A LAND manager of the parcel is its owner (the estate's owner
  when the parcel sits in an estate), its operator (the estate's operator
  when the parcel sits in an estate), its update operator (a cleared parcel
  update operator falls back to the estate's), an
  `UpdateManager` of the owner or an `ApprovalForAll` operator of the
  owner. A collaborator is a wallet in the active deployment's
  `metadata.logsPermissions`.
- **Routes.** A LAND manager reaches every owner and deployer route: the
  world and player values (read, write, delete, list and the clears),
  `GET /players` and its clear, `/usage/world`,
  `/usage/players/{address}`, `/usage/env`, `POST /exports`, and the env
  writes, key list and clear. A collaborator reaches the per-key and list
  routes of world and player values, `GET /players` and `POST /exports`,
  never a clear, `/usage` or `/env`. Neither role reads an env value:
  `GET /env/{key}` admits only the authorized addresses and the scoped
  delegation. `GET /collaborator` lists the Genesis scenes whose
  `logsPermissions` name the signer and that pass the collaborator rule
  below; a LAND manager missing from `logsPermissions` is not listed.
- **Manager sources.** The LAND authorization index alone. Until it has
  a published fold (below), storage management answers 503 `LAND rights
  are unavailable on this node: the land-authz index has no fresh
  published fold`, and neither `COMMS_LAND_OPERATORS_URL` nor a subgraph
  stands in (`catalyst serve` always attaches the index; the URL is read
  only by a permission core without one). A read of the index that fails
  answers 401 `Unauthorized: Failed to verify world permissions` before
  `logsPermissions` is consulted.
- **Collaborator source.** The index alone. Without a published index
  `GET /collaborator` lists no Genesis scene.
- **Index publication.** `LAND_AUTHZ_INDEX_ENABLED=true` with
  `ETH_RPC_URL` runs the indexer in `catalyst index` (`NODE_POSTURE=mirror`
  turns the flag on); `catalyst serve` reads the published index from the
  shared database whatever its own flag says. A pass runs every
  `LAND_AUTHZ_INDEX_INTERVAL_SECS` (120) and publishes a fold within 25
  blocks of a head kept `LAND_AUTHZ_CONFIRMATIONS` (12) blocks back. The
  managers read the index while its fold is current (folded at the walk's
  head, or no authorization event walked past it), at most 30
  minutes old on the database clock and its registries are the node's; a
  fold older than that makes them unavailable at once.
  The lamb2 LAND routes require a fold at most ten minutes
  old. The index interval is capped at four minutes. The batch permissions
  preview uses the same 30-minute window as deploy authorization. They
  answer the 503 `land-authz-index` envelope when the fold expires; no
  subgraph stands in. `GET /lambdas/land-authz/status`
  reports Lambdas readiness as `ready`, with `readyWindowSecs` and
  `cursor.foldAgeSecs`, `cursor.foldedBlock` and `cursor.lastBlock`.
- **Collaborator freshness.** Collaborator readiness is a separate
  condition from status `ready`. The collaborator rule needs a full replay
  started (`cursor.fullReplayStarted`), a published fold
  (`cursor.foldedBlock` not null) at most 30 minutes old, registries
  matching the node's and `history_timestamps_ready` (every indexed log
  carries its block time). It does not need `cursor.foldedBlock` to equal
  `cursor.lastBlock`. The status route does not report
  `history_timestamps_ready`; the admin `GET /admin/api/mirrors` returns
  the whole `land_authz.index_cursor` row under `body.landAuthz`. For
  every parcel of the scene, the latest LAND transfer, the latest estate
  membership change and the estate's latest transfer must be at or below
  the fold and more than 15 minutes older than the deployment's signed
  timestamp. That timestamp must not be ahead of the database clock, and
  the deployer refuses one more than 15 minutes ahead of the node's clock
  (`REQUEST_TTL_FORWARDS_MS` in `content/src/write_deployer.rs`). A parcel
  the index does not know, or one held by the zero address, refuses the
  scene; a transfer newer than the fold refuses it until folded. A scene
  deployed less than 15 minutes after a transfer stays refused, since its
  timestamp never changes: only a new deployment admits its collaborators
  again.

### HTTP

- **Deeply nested values.** A value nested deeper than about 4,500 levels
  is accepted and stored, up to the depth Postgres' `jsonb` input accepts
  (then 500 `{"error": "Internal Server Error"}`). Upstream answers 500 from
  that depth on, where ajv's recursive validation and `JSON.stringify`
  exhaust Node's stack.
- **Schema errors of deep values.** When the non-finite number that fails
  the value's `anyOf` sits more than 128 containers deep, or its error paths
  exceed the 1 MiB budget, `data` holds only the outermost `anyOf` error at
  `/value`. The budget counts six bytes per UTF-16 unit across seven path
  copies per level, so a failing path over about 25,000 units also triggers
  this fallback. Upstream lists every level's
  errors, a body that grows with the square of the depth (6.7 MB at 1,000
  levels).
- **NUL in a query.** A `%00` in the query string (a list's `prefix`)
  answers 400 `{"error": "request path or query contains a NUL byte"}`
  before the signed fetch is checked. Upstream passes it to Postgres and
  answers 500. A NUL in a key answers upstream's 400 after authorization.
- **`GET /collaborator` lists local scenes only.** Mirrored world scenes
  are left out, as their `logsPermissions` grant no storage access on this
  node; upstream lists every scene its deployment consumer indexed.
- **Status checks.** `/health/live`, `/health/ready` and `/health/startup`
  answer as upstream's status check component: live answers `alive`, ready
  and startup a static pass of the `server` component
  (`{"details":{"server":{"status":"pass"}},"status":"pass"}`).

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
  caps, logging and the gzip stream otherwise follow upstream. The SQL statement
  timeout follows `PG_COMPONENT_STREAM_QUERY_TIMEOUT` (zero disables it).
  The idle-transaction guard follows `EXPORT_TIMEOUT_SECONDS` independently,
  clamped to 1–2,147,483,647 ms. Both settings are transaction-local.
- **Metrics.** The worlds routes record `http_requests_total`,
  `http_request_duration_seconds` and `http_request_size_bytes` by
  `method`, `handler` and `code`, as upstream's defaults; `handler` is the
  matched route as this node mounts it (for example
  `/world-storage/values/{key}`, upstream `/values/:key`), shared with the
  node's other routes. The logger and pg metric families are not exported.
- **Cache staleness (shared with upstream).** The single-value LRU is per
  process; with several storage-serving processes, a read may be up to
  `STORAGE_CACHE_TTL_SECONDS` stale after another process's write. Within
  one process, a generation check prevents an in-flight read from repopulating
  a value invalidated by a completed write or delete. An unrelated invalidation
  may also discard a pending fill; it does not evict unrelated cached values.
- **Encryption key.** `ENCRYPTION_KEY` is required, as upstream. This node
  has no committed development key, so upstream's production refusal of
  that key has no counterpart.
