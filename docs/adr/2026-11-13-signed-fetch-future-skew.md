# ADR: Signed fetch accepts timestamps at most 60 seconds ahead

- Status: Proposed
- Date: 2026-11-13

## Context

A signed-fetch request carries `x-identity-timestamp`, and the payload binds
the method, path, timestamp and metadata, not the body or host. The node had
no replay cache; freshness was the only bound. The pinned
`@dcl/crypto-middleware` accepts a timestamp within five minutes on either
side of the server clock, so a request dated five minutes ahead stays usable
for ten. Comms already accepted only 60 seconds either way.

## Decision

`check_timestamp_window` (`library/dcl-auth/src/signed_fetch.rs`) keeps each
surface's past window (five minutes by default) and caps the future side at
`MAX_FUTURE_SKEW_SECS`, 60 seconds, on every surface that verifies a signed
fetch: the node's routes and sockets and the SDK's preview server.

## Consequences

- A captured request is usable for at most the past window plus 60 seconds.
- A client whose clock runs more than 60 seconds fast is refused with the
  surface's expired answer until its clock is corrected. Operators should
  watch the signed-fetch refusal rate after rollout.
- The surfaces diverge from the reference middleware on the future side.

## Alternatives

- Keep five minutes either way: matches upstream, leaves the ten-minute reuse.
- A replay cache keyed on the signer, timestamp and final signature: binds
  each request to one use, but needs shared state across replicas; left as
  follow-up work for state-changing routes.
