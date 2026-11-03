# Gateway

Nginx public entry for APIs and Sites (routing source).

The exact `/places/place?position=...` and `/places/world?name=...` endpoints, including their trailing-slash forms, stay backend social cards; `/places/place/<position>`, `/places/world/<name>` and their `.data` requests go to Sites, and the JSON catalogue stays under `/places/api`.

The exact `/events/assets/event-default.webp` path serves Catalyst's bundled event image. Event image links use the node's `PUBLIC_URL` plus the `/events` mount; `EVENTS_BASE_URL` remains the website URL used by sitemaps.

Set `PUBLIC_URL` to the externally reachable node base URL, including any deployment prefix. Without it, the node uses its bind address (for example `http://0.0.0.0:<port>`), which is only a local fallback and cannot provide public image or notification links. See [event client and deployment guidance](../../services/catalyst/docs/events.md).

```sh
nix build .#image-gateway --out-link .cache/gateway-image
docker load --input .cache/gateway-image
python3 infra/gateway/smoke.py --revision "$(git rev-parse HEAD)" --arch amd64   # or arm64
docker run --rm --network dcl-one --read-only --cap-drop ALL --security-opt no-new-privileges \
  --tmpfs /tmp:rw,nosuid,size=512m -p 8080:8080 -e GATEWAY_DOMAIN=example.org \
  quay.io/decentraland/dcl-one-gateway:main
```

| Setting | Default | Meaning |
| --- | --- | --- |
| `GATEWAY_DOMAIN` | `localhost` | Base domain |
| `GATEWAY_HOST_ALIASES` | empty | Extra apex hosts |
| `GATEWAY_APIS_UPSTREAMS` | `apis:5210` | API destinations |
| `GATEWAY_SITES_UPSTREAMS` | `sites:5212` | Sites destinations |
| `GATEWAY_ADMIN_CIDRS` | empty | Operator networks; empty denies all |
| `GATEWAY_TRUSTED_PROXY_CIDRS` | empty | Proxies trusted for client IP and scheme |
| `GATEWAY_STORAGE_TRUSTED_CIDRS` | empty | Client networks (such as scene-host egress) with a higher, bounded storage budget |
| `GATEWAY_RESOLVERS` | resolv.conf | DNS servers |
| `GATEWAY_STORAGE_ROOT` | empty | Content mount; API sets `STORAGE_X_ACCEL_BASE=/__protected_storage` |

- Publish only port 8080; keep APIs and Sites private; terminate TLS before it.
- Operator paths (`/admin`, `/server`, `/metabase`, telemetry admin) are gated on the normalized path; an allowed network does not authenticate.
- Never trust arbitrary client networks as proxies; a trusted proxy must set `X-Forwarded-For` and `X-Forwarded-Proto`.
- The gateway overwrites client IP, forwarding, `X-Catalyst-Admin-Gate` and `x-original-path` headers and removes `True-Client-IP`.
- `/internal/pulse` and `/metrics` are not public; internal callers use the APIs' private address.
- Limits: bodies 1 MiB (deployments 200, builder 21, reports 11); 30 r/s per client (content 100, deployments 2); `%3F` in paths returns 400.
- `/world-storage` (after host rewriting): reads 100 r/s per client (burst 400), writes also 30 r/s (burst 60). Each client address in `GATEWAY_STORAGE_TRUSTED_CIDRS` gets a separate 1000 r/s budget (burst 1000); the allowance is per address, not per CIDR, and never disables limiting. Startup rejects overlaps with `GATEWAY_TRUSTED_PROXY_CIDRS`.

A service subdomain `<sub>.<domain>` reaches the APIs under `/<sub>`, except
upstream host names served under another prefix: `abcdn` and `abgen-cdn` →
`/ab-cdn`, `asset-bundle-registry-abgen` → `/asset-bundle-registry`,
`rpc-social-service-ea` → `/social-rpc`, `market` → `/shop`, `storage` →
`/world-storage`, `comms-gatekeeper-local` → `/comms-gatekeeper`; `peer` and
`worlds` are the root. `gateway.<domain>/<sub>/...` uses the same table, and a
path already under the target prefix is not prefixed twice.
