# Native Explorer gateway routing and HTTP inspection

- Status: Accepted
- Date: 2026-11-03

The desktop operator can opt into first-party service routing with `--gateway`. Explicit service
URLs take precedence. The engine resolves the destination before signing. On a gateway virtual
host the signature covers the path forwarded to the service. On a direct origin it covers the
public path. The engine supplies the native HUD with resolved service bases stripped of credentials.

The native HUD bridge can use a private configured gateway for a signed, supported API route.
A grant matches one method, URL and set of engine-issued identity headers. The bridge consumes
the grant at dispatch, restricts additional headers and rejects redirects. Ordinary scene traffic
continues through its scene permission and public-address checks.

`--network-devtools` opens a loopback-only, read-only Chrome DevTools Network target. A content
attempt gets a trace after its scheduler issues the grant. That trace follows the response until
the body completes, fails or is dropped. Only current subscribers receive events, with recognized
route labels and transfer lengths. Headers, bodies and dynamic URL components stay private.
