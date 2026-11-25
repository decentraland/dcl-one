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
(`library/explorer-core/lib/gateway.ts`) and pass the gateway to the engine. Requests the HUD
signs itself sign the path a `gateway.` virtual host forwards, as the engine does. Feature flags go
through the gateway too, as with `--gateway`: the gateway's copy is one document for everyone, so
such a session loses per-player rollout groups unless the flags service is passed explicitly.
`/play` without a gateway keeps reading that service directly. The cross-language
route test compares the HUD resolvers with the engine's for the virtual-host and apex layouts.

Trust follows the base domain:

- Only an origin is trusted, never a path under one: a node proxies other hosts under
  `/_scene/<host>` with the request's signature headers.
- On web, any gateway other than `https://gateway.<base>`, with no port or
  path, where `<base>` is the session's base domain and one of `decentraland.org` and
  `decentraland.zone`, stops the boot on the untrusted-launch prompt. `/play` also trusts its own
  origin. The apex of those domains is the website, not a gateway, so it is not trusted; an apex
  gateway elsewhere takes `--gateway` or the prompt.
- The native client takes from a link only such a gateway, because a link starts it without a
  prompt. A link with any other gateway stops the launch with a usage error rather than starting
  without it, so the session never runs against a destination the link did not name. Any other
  gateway needs `--gateway`, which wins over the link.
- The `/open_deep_link` test command refuses a link whose gateway differs from the one the client
  started with.
