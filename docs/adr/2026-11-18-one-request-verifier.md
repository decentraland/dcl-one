# ADR: One signed-request verifier in dcl-auth

- Status: Proposed
- Date: 2026-11-18

## Context

A signed request carries an auth chain in `x-identity-auth-chain-N`, a
timestamp and a metadata object, signed over `method:path:timestamp:metadata`.
The node verifies one on HTTP routes, on WebSocket auth frames and in the Pulse
handshake. Each surface assembled its own pipeline from the steps in
`dcl_auth::signed_fetch`, so stage order, accepted forms and HTTP answers
differed between them and from the reference, `@dcl/crypto-middleware` 6.5.0
over `@dcl/crypto` 3.9.0. The reference also added scoped logins (ADR-162): a
delegation that grants an ephemeral key only named operations of named
services.

## Decision

`dcl_auth::request::verify_request` is the one verifier for signed requests.
Every surface that takes one moves onto it; until then the earlier steps stay
available.

- **Input.** `RequestParts` holds the method, the signed path
  (`SignedPath`), `x-original-path`, the raw chain link values, the raw
  timestamp and the optional raw metadata. It is built from an axum
  `HeaderMap` or from the fields of a WebSocket frame or a Pulse handshake.
  Header bytes read as Latin-1 and repeated values join with `", "`, as Node
  reads them.
- **Options.** `VerifyOptions` is built once and refuses a bad configuration:
  the expiration window (`SIGNED_FETCH_WINDOW_SECS`, 60 s, by default; it
  bounds the past, and the future up to the 60 s `MAX_FUTURE_SKEW_SECS` cap of
  the [signed-fetch window ADR](2026-11-13-signed-fetch-window.md)),
  the maximum chain length (10), `canonical_metadata_keys`, a
  `metadata_validator` (`require_signer`, `reject_if_signer`,
  `require_canonical_field`, `canonical_field`), the service, the scoped-login
  lifetime cap (24 h), the route's operation and resource, `optional`, and the
  EIP-1654 validator.
- **Stages and answers** follow the reference: chain headers (400), timestamp
  (400 malformed, 401 outside the window), metadata (400), the validator
  (400), the signature (401; 503 when a contract wallet cannot be asked). On
  any 401, with `canonical_metadata_keys`, the key guard (400) and then the
  pre-6.0.0 payload. A scoped login is checked against the route's operation
  and resource (403 with `SCOPED_OPERATION_REQUIRED`,
  `SCOPED_RESOURCE_REQUIRED`, `SCOPED_OPERATION_DENIED` or
  `SCOPED_RESOLUTION_FAILED`). The result is the lowercased signer, the
  metadata object, the scope and the authorized resource.
- **Scoped logins** are accepted only by a verifier configured with a
  service, and only when they grant it something. Every other verifier
  refuses them: content deployments, synced and peer entities, lambdas
  `validate-signature`, the Pulse handshake and the archipelago challenge.
  Routes get service and operation names when they move onto the verifier.
- **Contract wallets.** A validation makes at most 16 JSON-RPC calls of 5 s
  each within 15 s. A call over the budget is an invalid signature; a node
  error, a timeout, a redirect, an HTTP error status or a malformed answer
  (an `eth_call` result that is not one ABI word of hex) ends the validation
  as unavailable; for every other caller an HTTP error status or a missing
  result is a failed call, never an answer. A revert is the contract's refusal: code 3
  or -32015, or an execution-failure message. Every caller of the validator
  reads responses up to 1 MiB and gives it a client that follows no redirect,
  so a call never reaches another node.
- **Login purpose.** A signed request is a login: every ephemeral
  delegation, ECDSA or EIP-1654, must start with `Decentraland Login`
  ([login delegation ADR](2026-11-19-login-delegation-purpose.md)); any other
  first line is an invalid signature (401), with the same key guard and
  pre-6.0.0 payload steps as any other 401. A scoped link carries its own
  `Decentraland Scoped Login` line.
- **Expirations** read as `Date.parse` reads them
  (`dcl_auth::js_date`, checked against Node over a fixture corpus). A scoped
  login's expiration must be the exact `toISOString` form.

### Deliberate differences from the reference

- `x-original-path`: the signature may be over the gateway path when it ends
  with the route, since the gateway mounts services under prefixes and
  clients sign the public path.
- Chain rules: only delegations sit between the SIGNER and the last link
  (ADR-162's scoped structure aside), high-s signatures are refused, and
  addresses `0x00..00` to `0x00..ff` are never asked as contract wallets.
- A delegation's first line must be `Decentraland Login`; the reference
  reads only its address and expiration lines.
- The timestamp is digits only; any other form is 400. The reference reads it
  with `Number()` until decentraland/core-libs#83 lands, after which it is
  digits only too.
- The future side of the timestamp window stops at 60 s whatever the window;
  with the default 60 s window this is the reference's bound.
- A date-time expiration without an offset reads as UTC; the servers run in
  UTC, where the reference reads it the same way.
- A contract wallet's acceptance at the latest block is reused for 60 s, and
  one at a given past block for 1 h; refusals and failures are not cached.

## Consequences

- One stage order and one set of answers for every surface once it moves.
- Stored entities keep their own rules: content deployments and sync accept
  ECDSA signatures without `0x` and delegations of any purpose.
- Every chain walker other than content sync now reads an expiration with
  `Date.parse` first, then the shapes it read before, so each accepts the
  forms `Date.parse` reads that it used to refuse: local content and worlds
  deployments, worlds mirroring, signed-fetch routes not yet on the verifier
  (comms, worlds storage, shop lambdas), the archipelago and Pulse logins,
  explore's auth API, the social webhooks and the scene CLI. Content entities
  synced from a peer read only the earlier shapes and take any other
  expiration on the peer's word, as before.
- `fixtures/request-vectors.json` holds requests signed by the reference
  clients with the reference's answers; regenerating it with
  `fixtures/gen-request-vectors.js` re-checks parity against a new release.

## Alternatives

- Keep per-surface pipelines: each surface drifts, and scoped logins would be
  added several times.
- Ask the catalyst's `validate-signature` for contract wallets, as the
  reference does without a provider: adds an HTTP hop to every such request.
