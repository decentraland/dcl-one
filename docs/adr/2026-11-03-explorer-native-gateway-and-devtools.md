# Native Explorer gateway routing and HTTP inspection

- Status: Accepted
- Date: 2026-11-03

The desktop operator can opt into first-party service routing with `--gateway`. Explicit service
URLs take precedence. The engine resolves the destination before signing. On a gateway virtual
host the signature covers the path forwarded to the service. On a direct origin it covers the
public path. The HUD bridge reads the engine's resolved service bases through
`getServiceUrls()`. The service map exposed to the HUD omits credentials. `Reels` identifies
the public photo share page and `CameraReel` identifies the API that serves photo data.

Private deployments can use the native HUD outside local preview. The super-user bridge
takes its permitted service bases from the engine's reply. On a supported API route it
can consume a grant for one method, URL and set of engine-issued identity headers.
The transport restricts additional headers and rejects redirects. Ordinary scene traffic
continues through its scene permission and public-address checks.

`--network-devtools` opens a loopback-only, read-only Chrome DevTools Network target. A content
attempt gets a trace after its scheduler issues the grant. That trace follows the response until
the body completes, fails or is dropped. Only current subscribers receive events, with recognized
route labels and transfer lengths. Headers, bodies and dynamic URL components stay private.
