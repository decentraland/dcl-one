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
- **Wallet storage allowance.** `GET /wallet/{wallet}/stats` reports
  `maxAllowedSpace` as one world-size cap (the publish limit, smaller for a
  personal world) per DCL world the wallet owns: the NAMEs the names
  authority says it holds now, plus its personal world when personal worlds
  are enabled. The upstream service derives it from an account-holdings
  service. `dclNames` and `ensNames` list the wallet's worlds that have a
  world row here; a DCL world counts when the names authority names the
  wallet as its holder (the stored owner when no names reader is
  configured). `usedSpace` sums the stored size of every scene in the
  wallet's DCL worlds, mirrored copies included.
- **Error envelope on 403 and 500.** Worlds routes answer
  `{error, message}` with the @dcl/http-commons labels (`Bad request`,
  `Not Authorized`, `Not Found`). A 403 carries the message in `error` as
  the upstream handlers' own 403s do, and also in `message`. A 500 is
  `{"error": "Internal Server Error"}`.

## Comms

- **Realm name.** `WORLDS_REALM_NAME_STRIP_ENS` (default `true`) strips
  `.dcl.eth` from `/about`'s `realmName` for worlds published here, so a
  stock client does not resolve them against Decentraland's registry.
  Mirrored worlds keep the suffix.
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

## Permissions

- **Community ids** in an allow-list must be UUIDs.
- **Removing a grant's last parcel** deletes the grant. Upstream keeps the
  row, which then becomes world-wide.
- **Streaming permission** is enforced when a participant asks to stream,
  not only recorded.
- **Shared-secret hashes** use bcrypt cost 12 (upstream: 10); existing
  hashes verify unchanged.
