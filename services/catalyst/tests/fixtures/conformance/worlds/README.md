# Worlds content server: replacement backlog

These files compare every endpoint of the Decentraland worlds content server with what a node built
from this tree (`catalyst serve`) answers at the same method and path. The worlds surface is served by
the `worlds` crate at the root and under `/worlds-content-server`; the comms handshakes, the LiveKit
webhook and the presence reads are served by the `comms` crate (gatekeeper and presence, with room
occupancy from Pulse and LiveKit).

The target is a transparent replacement: given the same data, each method, path, status, body field,
header and error branch should be the same, so the explorers, the builder, creator-hub, the CLI, the
admin tooling and LiveKit can be pointed at this tree instead of the upstream service. Every row was
reached by reading both trees; nothing here was captured from a live server or run, unless a row says
otherwise.

| upstream | revision |
| --- | --- |
| `github.com/decentraland/worlds-content-server` | `ebbf8d41d07973e317892067c765107d3512938e` (2026-11-12) |
| this tree | the revision that contains this file |

Upstream packages that decide rows were read from their published tarballs at the versions upstream's
`yarn.lock` pins (never run): `@dcl/http-commons` 2.0.0, `@dcl/http-server` 2.3.0,
`@dcl/crypto-middleware` 6.3.0, `@dcl/schema-validator-component` 1.0.1, `@dcl/schemas` 27.1.0 and
`livekit-server-sdk` 2.15.0. Each file says which it read.

## Verdicts

The verdict words are the ones of [`../content-lambdas/README.md`](../content-lambdas/README.md):
MATCH, GAP, DIVERGENT and MISSING. A row whose difference is listed in
[`docs/worlds-compatibility.md`](../../../../docs/worlds-compatibility.md) is marked "intended", and the
row says whether it breaks a drop-in replacement.

## Files

Upstream's `src/controllers/routes.ts` registers 39 routes (`/reprocess-ab` and `/gc` only when
`AUTH_SECRET` is set; the http-server `/health/*` and `/metrics` routes are not counted). The files count
40 endpoints: `POST /entities` is two, the single request and `partial=true`. GET and HEAD of
`/contents/:hashId` and `/ipfs/:hashId` are separate endpoints described in one section.

| file | area | endpoints | MATCH | GAP | DIVERGENT | MISSING | backlog |
| --- | --- | --- | --- | --- | --- | --- | --- |
| [worlds-deploy.md](worlds-deploy.md) | `POST /entities` (single and `partial=true`), `DELETE /entities/:world_name`, `DELETE /world/:world_name/scenes/:coordinate` | 4 | 0 | 0 | 4 | 0 | WD-01..28 |
| [worlds-reads.md](worlds-reads.md) | `/contents/:hashId`, `/ipfs/:hashId` (GET/HEAD), `/available-content`, `POST /entities/active`, `/index`, `/status`, `/reprocess-ab`, `/gc`; mounts, 404/405, CORS | 10 | 0 | 0 | 10 | 0 | WR-01..37 |
| [worlds-world.md](worlds-world.md) | `/world/:n/{about,scenes,settings,manifest,limits}`, `/worlds`, `/wallet/:w/stats`, `/wallet/contribute` | 10 | 1 | 0 | 9 | 0 | WW-01..42 |
| [worlds-permissions.md](worlds-permissions.md) | the ten `/world/:n/permissions*` routes | 10 | 1 | 1 | 8 | 0 | WP-01..21 |
| [worlds-comms.md](worlds-comms.md) | `/worlds/:w/comms`, `/worlds/:w/scenes/:id/comms`, `/get-comms-adapter/:roomId`, `/livekit-webhook`, `/live-data`, `/wallet/:w/connected-world` | 6 | 0 | 0 | 6 | 0 | WC-01..28 |
| **total** | | **40** | **2** | **1** | **37** | **0** | **156** |

Every upstream route has a route at the same method and path in this tree. At the root host, nine
worlds handlers answer only under `/worlds-content-server`. At the root,
`POST /entities`, `DELETE /entities/:world_name`, `POST /entities/active`, `/contents/:hashId`,
`/available-content` and `/status` are the catalyst content server's; `/ipfs/:hashId`, `POST /gc` and
`POST /reprocess-ab` have no root route (`worlds/src/core/mod.rs` `router`, `root_router`; `worlds/src/lib.rs` `app_router`). The gateway maps the
worlds host onto the prefix (WD, WR-12, WR-23, WR-27).

## P0 and P1

No item is P0 or P1. `POST /reprocess-ab` (WR-22) hands scenes to the local abgen instead of SNS,
an intended difference.

## Themes that cut across files

- **Signed-fetch refusals** (WD-02, WP-01, WW-24, WC-01): one mapper, `comms/src/signed_fetch.rs`
  `map_auth_error_api`, answers every signed worlds route with the crypto-middleware 6.3.0 text and the
  ADR-44 line. `PUT /settings` reads its multipart body only after the signed fetch (WW-24).
- **Validator bodies** (WP-02, WW-15): the schema-validated routes answer upstream's
  `{ok:false,message,data}`, built with the content crate's validator (`content::validation`, CR-2).
- **Absent vs `null`** (WP-03, WR-15, WR-18): these bodies write `null` or leave the key out as
  upstream does; no open item remains.
- **Name deny list** (WD-28, WR-35, WP-11, WP-20, WC-28): one `NameDenyList` carries
  `BLOCKED_WORLD_NAMES` and the DAO `banned-names` list to every consumer upstream has: deploy
  validation, `/entities/active`, the world reads and listings, the permission routes and the comms
  mints. After a failed list read only the configured blocks apply for five seconds here, while
  upstream caches an empty list for an hour (WR-35, open).
- **Multipart limits and texts** (WD-04..11, WW-21..23): both multipart routes answer upstream's caps,
  duplicate-field rule, 413 and timeout texts, apart from the intended WD-08, WD-09 and WD-11.
- **CORS** (WR-06, WW-37, WR-32): every worlds router carries the layer (WR-06; the mints, WC-02);
  the origin is echoed with credentials where upstream answers `*`.
- **Intended differences** (WD-03, WD-19, WW-26, WP-13, WR-23): the write switch, events not published
  on SNS and root mount collisions are listed in `docs/worlds-compatibility.md`.
