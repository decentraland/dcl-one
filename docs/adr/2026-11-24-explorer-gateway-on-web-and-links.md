# Explorer gateway on web and in links

- Status: Accepted
- Date: 2026-11-24

## Context

The [native gateway](2026-11-03-explorer-native-gateway.md) routes first-party HTTP APIs through
one origin and signs the path that origin forwards. The web client and `decentraland://` links
need the same selection, with the same endpoints and signing paths.

A gateway receives every signed API request of the session. Selecting one is as sensitive as
selecting the base domain.

## Decision

`gateway` is a shared launch option: the native flag, the web `?gateway=` param and a link's
`gateway=` key. The engine latches it with the base domain and resolves, signs and rewrites
profile images exactly as `--gateway` does. Explicit service URLs take precedence.

The engine's service table carries each service's gateway segments (`Service::gateway_segment`).
Both HUD hosts join them with the gateway for their own requests
(`library/explorer-core/lib/gateway.ts`) and pass the gateway to the engine. The cross-language
route test compares the HUD resolvers with the engine's for the virtual-host and apex layouts.

Trust follows the base domain:

- Only an origin is trusted, never a path under one: a node proxies other hosts under
  `/_scene/<host>` with the request's signature headers.
- On web, any gateway other than an HTTPS origin, with no port, under the session's base domain
  when that is `decentraland.org` or `decentraland.zone` stops the boot on the untrusted-launch
  prompt. `/play` also trusts its own origin.
- The native client takes from a link only such an origin, because a link starts it without a
  prompt. Any other gateway needs `--gateway`, which wins over the link.
- The `/open_deep_link` test command refuses a link whose gateway differs from the one the client
  started with.
