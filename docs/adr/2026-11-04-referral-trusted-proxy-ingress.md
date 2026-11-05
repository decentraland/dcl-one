# ADR: Referral client IP from trusted proxies only

- Status: Proposed
- Date: 2026-11-04

## Context

Referral attribution limits how many invited users one referrer may gain from
one client IP. The pinned `social-service-ea` (`src/utils/client-ip.ts` at
`b6b2cef`) takes the first of `CF-Connecting-IP`, the rightmost
`X-Forwarded-For` entry and `X-Real-IP`, from any client. In the catalyst
binary the Social routes are reachable directly and through the gateway, so a
client that chooses those headers chooses its quota bucket.

## Decision

The referral quota uses the TCP peer address. Only a peer inside
`HTTP_TRUSTED_PROXY_CIDRS` (comma-separated networks, empty by default) may
supply the client address, and only as one valid `X-Real-IP` value, which that
proxy must overwrite from its own validated client address.
`CF-Connecting-IP` and `X-Forwarded-For` are never read. Addresses are stored in
canonical form; historical values compare as inet, so mapped IPv4 and
differently spelled IPv6 aliases share a quota. A request without a peer
address answers 400 "Unable to determine client IP address".

`referral_settings` in `serve.rs` passes the setting to the Social referral
component; `configured_proxies_reach_the_referral_client_ip_path` checks the
setting through to the stored client IP over a real socket.

## Consequences

- A deployment behind a gateway or load balancer must list that proxy's
  networks (not its upstream CDN's) and make it overwrite `X-Real-IP` before
  gateway cutover. Otherwise every client behind it shares the proxy's quota.
- Direct clients cannot pick a bucket with forwarding headers.
- The answers for clients behind an unlisted proxy differ from the source,
  which would have read their forwarding headers.

## Alternatives

- Read the source's header chain unchanged: lets any client choose its bucket
  on a directly reachable node.
- Trust `X-Forwarded-For` from listed proxies: needs a hop count per
  deployment and parses a client-extendable list; one overwritten header does
  not.
- Trust every private network by default: wrong for nodes whose clients share
  a private network with the node.
