# ADR: Signed fetch accepts timestamps at most 60 seconds old or ahead

- Status: Accepted
- Date: 2026-11-13, window decided 2026-11-19

## Context

A signed-fetch request carries `x-identity-timestamp`, and the payload binds
the method, path, timestamp and metadata, not the body or host. The node has
no replay cache; freshness is the only bound on how long a captured request
stays usable. `@dcl/platform-crypto-middleware` and the pinned
`@dcl/crypto-middleware` both default to a 60-second expiration
(`DEFAULT_EXPIRATION`, `1000 * 60`), which most signed services used,
including events, shop, credits and the marketplace trades and coupons
routes. A few services configured wider windows (marketplace favorites five
minutes, builder 30 minutes). `@dcl/platform-crypto-middleware` never refuses
a future timestamp; `@dcl/crypto-middleware` refuses one more than its
expiration ahead.

## Decision

Every signed route the node serves uses one freshness window,
`SIGNED_FETCH_WINDOW_SECS` (`library/dcl-auth/src/signed_fetch.rs`), 60
seconds: the shared verification profile (explore, places, events, shop,
credits, marketplace, social and builder routes), the comms gatekeeper,
including its authoritative-server route, the comms and worlds storage
signed-fetch surfaces and the worlds scene-state socket. There is no
per-route override.

`check_timestamp_window` caps the future side at `MAX_FUTURE_SKEW_SECS`, 60
seconds, on every surface that verifies a signed fetch: the node's routes and
sockets and the SDK's preview server. `check_past_window`, which camera reel
uses for its 60-second past-only window, applies the same future cap.

## Consequences

- A captured request is usable for at most two minutes around its timestamp.
- Routes that used a wider window (favorites, builder) now refuse a request
  signed more than 60 seconds earlier; clients sign each request when it is
  sent.
- A client whose clock is more than 60 seconds off is refused with the
  surface's expired answer until its clock is corrected. Operators should
  watch the signed-fetch refusal rate after rollout.
- The future cap matches `@dcl/crypto-middleware` at its default expiration
  and is stricter than `@dcl/platform-crypto-middleware`, which had none.
- Outside this decision: the social RPC socket handshake reads its window from
  operator configuration (`AUTH_WINDOW_SECS`, default 300 s, which also sets
  its first-frame deadline, or `WS_AUTH_SIGNATURE_EXPIRATION_IN_MS`), and the
  SDK's local preview server keeps its own tolerance.

## Alternatives

- Keep five minutes for every route: tolerates more clock drift, leaves each
  captured request usable for longer than the 60 seconds most services used.
- 60 seconds on money-moving and write routes only, wider where a service
  used a wider window: keeps per-route exceptions for no client that needs
  them.
- A replay cache keyed on the signer, timestamp and final signature: binds
  each request to one use, but needs shared state across replicas; left as
  follow-up work for state-changing routes.
