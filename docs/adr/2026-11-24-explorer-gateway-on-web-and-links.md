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

Both HUD hosts resolve their own requests with a copy of the engine's mapping
(`library/explorer-core/lib/gateway.ts`) and pass the gateway to the engine. The cross-language
route test compares the two resolvers for the virtual-host and apex layouts.

Trust follows the base domain:

- On web, a gateway outside `decentraland.org` and `decentraland.zone` over HTTPS stops the boot
  on the untrusted-launch prompt. `/play` also trusts its own host.
- The native client takes from a link only an HTTPS gateway under those domains, because a link
  starts it without a prompt. Any other gateway needs `--gateway`, which wins over the link.
- A running native client refuses a link whose gateway differs from the one it started with.
