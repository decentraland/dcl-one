# Native Explorer gateway routing

- Status: Accepted
- Date: 2026-11-03

The desktop operator can opt into first-party service routing with `--gateway`. Explicit service
URLs take precedence. The engine resolves the destination before signing. On a gateway virtual
host the signature covers the path forwarded to the service. On a direct origin it covers the
public path. The HUD bridge reads the engine's resolved service bases through
`getServiceUrls()`. The service map exposed to the HUD omits credentials. `Reels` identifies
the public photo share page and `CameraReel` identifies the API that serves photo data.

The bundled bridge and a system scene selected by the operator can use loopback and LAN
services outside preview. Their native Fetch uses the existing scene client and address
policy with private-network access enabled. Link-local and metadata addresses remain blocked.
The authoritative server retains its existing network policy. Ordinary scene traffic
continues through its scene permission and public-address checks.

A hostname beginning with `gateway.` selects the gateway virtual-host layout.
Other hostnames select the apex layout. This supports both deployment entry points while
signing the path the destination verifies.

Pulse and the WebSocket RPC services use their explicit service flags or base-domain
defaults. Storage also keeps its explicit service origin because its delegated signatures
are scoped to that origin. A single gateway flag does not redirect those transports.
