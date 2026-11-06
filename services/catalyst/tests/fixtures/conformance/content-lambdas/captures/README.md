# Live captures from a production Catalyst

Responses captured on 2026-11-04 (UTC) for the backlog rows that reading code could not settle, because
the behaviour lives in npm packages missing from the pinned upstream trees, or in production data.

## Sources

| source | what it is | revision at capture time |
| --- | --- | --- |
| `https://peer.decentraland.org` `/content/*` | the Decentraland content server | `/content/status`: `version` 8.3.2, `commitHash` `a416565db1e705cff9523540a2578af642598a29` |
| `https://peer.decentraland.org` `/lambdas/*` and `/about` | lamb2 | `/lambdas/status`: `version` 4.14.2, `commitHash` `02a37f5d88fb895339c20e84c2470be68cdf3f57` |
| `https://subgraph.decentraland.org/land-manager` | lamb2's default LAND subgraph (`src/ports/the-graph.ts`) | n/a |
| `https://subgraph.decentraland.org/marketplace` | lamb2's default names and LAND subgraph | n/a |
| `https://marketplace-api.decentraland.org` | lamb2's `MARKETPLACE_API_URL` default (`.env.default`) | n/a |

Both commit hashes are the upstream revisions the backlog files pin, so these captures describe the same
code the rows were read against.

The peer sits behind a CDN and an nginx gateway. Some answers come from those layers rather than from the
application: a capture whose response has no application trace header says so in its `note`. A
dcl-one node that replaces the peer replaces the application and the gateway, so those answers are still
what a client of the public peer sees.

## Method

- `curl -sS -D -`, one request per capture, a pause of 1 to 1.5 s between requests, no cookies or credentials.
- Read-only requests only: `GET`, `HEAD`, `OPTIONS`, a `PUT`/`PATCH`/`DELETE`/`FOO` that the router
  refuses with 405 (or the CDN with 400), and `POST` to the read endpoints `/content/entities/active`,
  `/lambdas/profiles` (empty `ids`), `/lambdas/crypto/validate-signature` (invalid or fake chains) and
  GraphQL queries. Nothing was deployed.
- The over-cap body for `POST /content/entities/active` was sent once (10 MiB + 512 bytes).
- The four captures without headers in the first pass were fetched again with headers; the bodies were
  byte-identical.

## File format

One JSON file per request, at `<area>/<row-id>-<short>.json`, named after the backlog row it settles:

- `captured_at`: the UTC date.
- `peer`: the base URL the `request.path` is relative to.
- `request`: `method`, `path` (with query), the `headers` that were sent, and `body`. A body that is not
  JSON, or too large to keep, is described in `<...>`.
- `response`: `status`, the kept `headers` (lower-case names), and `body`, parsed as JSON when it is
  JSON. `body_is_text: true` marks a body kept as text. A body described in `<...>` was elided.
- `note` (optional): what was cut or replaced, which layer answered when it was not the application, and
  observations from the same session that are not in the body.

JSON bodies are re-serialized with the key order and value types of the wire bytes. Whitespace is not kept.

## Sanitization

- Every 40-hex-digit address is replaced by a fake, `0x0000000000000000000000000000000000000010` upward,
  consistently across all files: the same real address always maps to the same fake. That includes
  contract addresses inside URNs and NFT ids. The zero address is kept. The fakes
  `0x0000000000000000000000000000000000000001` and `0x0000000000000000000000000000000000000002` were the
  values sent in the requests.
- 64-hex-digit order ids (`orders.id`, `activeOrder.id`) and the Catalyst list ids in `/contracts/servers` are
  replaced by fakes from `0x…0100` upward. Parcel token ids that encode to 64 hex digits (negative x) are kept.
- Names (`LL-7-*`) are replaced by `name-NN`, numbered by alphabetical order of the real names. Their
  token ids (label hashes, which reveal the name) are replaced by `1001`..., numbered by string order of
  the real ids. Relative order is therefore kept within those files.
- Wearable tags, the peer hosts listed by `/contracts/servers` (public `*.decentraland.org` hosts elsewhere are kept), the denylisted names, profile deployments and scene
  metadata are replaced or elided; each such file says so in `note`.
- Kept response headers: `content-type`, `cache-control`, `access-control-*`, `etag`, `retry-after`,
  `ratelimit-*`, `allow`, `vary`, `accept-ranges`, `content-encoding`, and `content-length` when the body
  is stored unchanged. Dropped: `date`, `server`, `set-cookie`, `cf-*`, `traceparent`,
  `strict-transport-security`, and `x-content-type-options: nosniff` (present on every captured response).
- Content hashes (CIDs), parcel coordinates and token ids derived from them, prices and timestamps are
  public chain or content data and are kept.
- The fakes keep real addresses out of the text; they are not anonymization. The kept public data
  (token ids, CIDs, coordinates, timestamps, prices) can be looked up on chain to recover the real
  address, and the same fake wallet is the subject of `LL-7-*` and `LI-18-*`.

## Not capturable here

- `@dcl/content-validator` rejection strings (CW rows): reaching them needs `POST /content/entities`,
  a write.
- An expired ephemeral key in `/lambdas/crypto/validate-signature` (LP-28): needs a chain signed by a
  real key.
- The application's answer to a method outside the router's list (CR-12, 501 upstream): the CDN answers
  `FOO` with its own 400 before the peer sees it.
- `Category` without a value in `/content/queries/erc721` (CS-16): needs a deployed wearable without a
  category.
