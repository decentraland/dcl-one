# Worlds: intended differences from worlds-content-server

The `worlds` crate serves the worlds content server's HTTP surface (at the
root and under `/worlds-content-server`) and its comms admission. The wire
contract follows
[`decentraland/worlds-content-server`](https://github.com/decentraland/worlds-content-server).
This page lists where the catalyst answers differently on purpose. A
difference not listed here is a gap, not a decision.

## HTTP

- **Invalid world names.** The permission, settings and manifest routes
  parse the world name first and answer 400 `Invalid world name: …` for a
  name that is not a valid world key. The upstream service answers defaults
  or 404 for such names.
- **Wallet storage allowance.** `GET /wallet/{wallet}/stats`, the publish
  size cap and `GET /world/{name}/limits` count only scenes published here:
  mirrored worlds and scenes are left out. Personal worlds are outside the
  allowance and are not listed. A wallet's DCL NAME worlds are the NAMEs it
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
  a failed lookup for 15 seconds; at most 8 lookups run at once.
  A publish credits back only the replaced scenes of a world charged to
  the same owner.
- **Legacy content keys.** The worlds `/contents/{key}` route also accepts a
  64-hex sha256 key and serves the blob whose raw-leaf CIDv1 has that
  digest, which covers files of one chunk (256 KiB) or less. An invalid key
  answers 404 (upstream: 400). Content types come from the catalyst's
  sniffer, so JSON is `application/json`.
- **Error envelope on 403 and 500.** Worlds routes answer
  `{error, message}` with the @dcl/http-commons labels (`Bad request`,
  `Not Authorized`, `Not Found`). A 403 carries the message in `error` as
  the upstream handlers' own 403s do, and also in `message`. A 500 is
  `{"error": "Internal Server Error"}`. The world comms mints
  (`/worlds/{name}/comms`, `/worlds/{name}/scenes/{id}/comms`) carry the
  reason in `error` for 401, 403, 404, 429 and 503, as upstream, and also in
  `message`; `/get-comms-adapter` does so for 429 only.

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
  the secret is checked and the count is cleared by a right secret. If the
  counter cannot be written the mint answers 503.
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
  401/403. The scene host's periodic re-admission keeps connected clients
  while the answer is undetermined. `GET /world/{name}/permissions` reports
  the live holder (no `owner` while the authority cannot answer), and the
  comms owner read for stream-key notices is unavailable then; neither
  falls back to the stored owner.
- **Capacity check fails closed.** When the live occupancy of a world cannot
  be recovered, the comms mint answers 503 instead of counting zero
  participants and admitting the user.
- **Scene room names.** A scene room is `w1:<base36 byte length of the world
  key>:<world key><scene id>`, with the scene id's case preserved. The
  prefix is fixed; there is no `SCENE_ROOM_PREFIX` or `COMMS_ROOM_PREFIX`
  setting. The world lobby room keeps `world-<name>`.
- **Room events.** Participant joins and leaves are published on the NATS
  subject `comms.room_events.v1`. No `peer.<id>.world.join|leave` subjects
  are published.
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
- **Newer-entity check** compares against the lower of the stored entity's
  timestamp and its row's update time, so a future-dated mirrored or
  imported entity cannot block redeploys.
- **Validation errors** report the first failure only, and scene schema
  errors name the failing path.
- **Grantee publishes** resolve the world's NAME holder live. If the names
  authority cannot answer, the publish is refused with 503.
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
- **`/gc`** reports `removedBytes` and has no `expiredUploads`. Every
  `/admin/*` route answers 401 for a missing or wrong token.
- **`GET /worlds` search** adds trigram similarity only when the `pg_trgm`
  extension is installed. `%` and `_` in the query match literally.

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

## Permissions

- **Community ids** in an allow-list must be UUIDs.
- **Removing a grant's last parcel** deletes the grant. Upstream keeps the
  row, which then becomes world-wide.
- **Streaming permission** is enforced when a participant asks to stream,
  not only recorded.
- **Shared-secret hashes** use bcrypt cost 12 (upstream: 10); existing
  hashes verify unchanged.
