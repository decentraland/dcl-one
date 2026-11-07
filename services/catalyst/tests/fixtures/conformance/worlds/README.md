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
| `github.com/decentraland/worlds-content-server` | `10af5255bb04a1a28e0c26bd190317c26a6015a9` (2026-10-21) |
| this tree | `2da8b8aea` (origin/main, 2026-11-07) |

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

Upstream registers 43 routes; the files compare them in 40 endpoint sections (GET and HEAD of a path
share a section where they share a handler).

| file | area | endpoints | MATCH | GAP | DIVERGENT | MISSING | backlog |
| --- | --- | --- | --- | --- | --- | --- | --- |
| [worlds-deploy.md](worlds-deploy.md) | `POST /entities` (single and `partial=true`), `DELETE /entities/:world_name`, `DELETE /world/:world_name/scenes/:coordinate` | 4 | 0 | 0 | 4 | 0 | WD-01..27 |
| [worlds-reads.md](worlds-reads.md) | `/contents/:hashId`, `/ipfs/:hashId` (GET/HEAD), `/available-content`, `POST /entities/active`, `/index`, `/status`, `/reprocess-ab`, `/gc`; mounts, 404/405, CORS | 10 | 0 | 1 | 8 | 1 | WR-01..34 |
| [worlds-world.md](worlds-world.md) | `/world/:n/{about,scenes,settings,manifest,limits}`, `/worlds`, `/wallet/:w/stats`, `/wallet/contribute` | 10 | 1 | 0 | 9 | 0 | WW-01..38 |
| [worlds-permissions.md](worlds-permissions.md) | the ten `/world/:n/permissions*` routes | 10 | 2 | 0 | 8 | 0 | WP-01..19 |
| [worlds-comms.md](worlds-comms.md) | `/worlds/:w/comms`, `/worlds/:w/scenes/:id/comms`, `/get-comms-adapter/:roomId`, `/livekit-webhook`, `/live-data`, `/wallet/:w/connected-world` | 6 | 0 | 0 | 6 | 0 | WC-01..27 |
| **total** | | **40** | **3** | **1** | **35** | **1** | **145** |

Every upstream route has a route at the same method and path in this tree except `POST /reprocess-ab`
(WR-22). `POST /entities` and `DELETE /entities/:world_name` answer only under `/worlds-content-server`
at the root host: at the root those paths belong to the catalyst content server, and the gateway maps
the worlds host onto the prefix (WD, WR-12, WR-23).

## P0 and P1

No item is P0. The P1 items:

| id | endpoint | what differs |
| --- | --- | --- |
| WD-03, WW-27 | every write route | `WORLDS_WRITES_ENABLED` is off by default, so publish, undeploy, settings and permission edits answer 503 with `Retry-After: 3600`; the switch is not in `docs/worlds-compatibility.md`. |
| WD-01 | `POST /entities` | A single-request deploy answers the request time as `creationTimestamp`, not the stored publication time. |
| WR-06 | `/worlds-content-server/{status,contents,ipfs,openapi.json}` | Added after the CORS layer: no CORS headers, a preflight answers 405. |
| WR-12 | `/about`, `/index` links | Content links point at the root `/contents`, the catalyst handler (no content-type sniffing, no sha256 keys, the content GET quota). |
| WR-17 | `GET /index` | The name deny list is not applied. |
| WR-18 | `GET /index` | `null` where upstream answers `''`, leaves the key out or answers `Date.now()`. |
| WR-22 | `POST /reprocess-ab` | Missing. |
| WW-01 | `GET /world/:n/about` | `scenesUrn` lists every deployed scene; upstream lists the newest. |
| WW-19 | `PUT /world/:n/settings` | A 400 answers `{error:"Bad request",message}`; upstream answers `{error:<message>}`. |

## Themes that cut across files

- **Signed-fetch refusals** (WD-02, WP-01, WW-24, WC-01, fixed): one mapper, `comms/src/signed_fetch.rs`
  `map_auth_error_api`, answers every signed worlds route with the crypto-middleware 6.3.0 text and the
  ADR-44 line. `PUT /settings` still reads its multipart body first (WW-24).
- **Validator bodies** (WW-15; WP-02 closed): schema-validated routes answer the worlds envelope where
  upstream answers `{ok:false,message,data}`; the parcel permission routes build that body with the
  content crate's validator (`content::validation`, CR-2).
- **Absent vs `null`** (WP-03, WW-04, WW-18, WR-15, WR-18): several bodies write `null` where upstream
  leaves the key out, or the reverse.
- **Multipart limits and texts** (WD-04..11, WW-21..23): field caps, duplicate fields, 413 vs 400 and
  timeout texts differ on both multipart routes.
- **CORS** (WR-06, WW-37, WR-32): one router misses the layer (WR-06; the mints have it, WC-02);
  elsewhere the origin is echoed with credentials where upstream answers `*`.
- **Undocumented intended differences**: the write switch, events not published on SNS (WD-19, WW-26,
  WP-13) and root mount collisions (WR-23) need lines in `docs/worlds-compatibility.md`.
