# Gateway feasibility and remaining gaps

The gateway is **`cloudflare-workers/workers/alb-router`**. The asset-bundle rewriter is outside this work. Follow the [decision summary](README.md): reproduce compatible operations, then synchronize only demonstrated residual outcomes.

The current gateway implementation is local `25e4193`, adding native vote ingress and optional per-attempt identity to the durable production-receipt flow. Generic read activation remains excluded. The branch is published in [MR !446](https://dcl.tools/ops/cloudflare-workers/-/merge_requests/446), with no deployment; published baseline routing is at `487a8f2dfa9bba7e5d607cb3b291d7c38e0aead2`.

## What exists

| Capability | Behavior and limit |
| --- | --- |
| Public host/prefix routing | [Router](https://dcl.tools/ops/cloudflare-workers/-/blob/487a8f2dfa9bba7e5d607cb3b291d7c38e0aead2/workers/alb-router/routing.ts) chooses one upstream. Explicit route attachments and existing-Worker bindings are separate configuration. Checked-in stack files do not establish live Cloudflare routes or middleware. |
| Read shadowing | [Shadow comparator](https://dcl.tools/ops/cloudflare-workers/-/blob/487a8f2dfa9bba7e5d607cb3b291d7c38e0aead2/workers/alb-router/shadow.ts) samples exact GET paths and compares bounded JSON. Writes, Upgrade, Range and body traffic are excluded. Production response remains authoritative. Read shadowing does not synchronize data or prove command compatibility. |
| Favorite/vote command ingress | `hybrid-production` admits operation IDs only for exact `PATCH /api/places/{UUID}/favorites` and `/likes` paths and sends the request to production. The source handler commits mutation, response and receipt; the coordinator later invokes native target execution. The same path now supports tri-state vote setters; other operations have no replay contract. |
| Full target routing | `dcl-one` mode transfers all matching requests, including writes and new WebSockets. It is a cutover control, not a safe read-only coexistence switch. No automatic write failback. |
| WebSockets | The original 101 and one upstream connection are preserved. Frames are not inspected, copied or replayed; existing connections stay on their original backend. |

The gateway does not provide generic durable command delivery by itself. A response or timeout cannot prove a source transaction committed. Stable retry identity, atomic source acceptance, target receipts, ordering and effect suppression belong to the operation contract.

## Blockers

1. **Public target runtime:** the [isolated follower](../services/catalyst/explore/src/migration_follower.rs) exposes private favorite verification/replay routes, not public Places/Events/Social APIs. The [normal runtime guard](../services/catalyst/src/migration_follower.rs) forbids ordinary serving while migration ownership exists. Add a deliberately isolated public reader only when its dependencies and effect restrictions are defined; do not remove the guard.
2. **Signed path compatibility:** [signed fetch](https://github.com/decentraland/decentraland-crypto-fetch/blob/9974951d10c0de581d1e5767231c096aa75c5900/src/signedFetchFactory.ts) signs the pathname and [source middleware](https://github.com/decentraland/platform-crypto-middleware/blob/e067d9636cbed1fb0738806b3a2810a86f85d898/src/index.ts) verifies the received path. `/api/places/...` → `/places/...` rewriting cannot preserve that signature. Hybrid mode rejects signed path changes; other routing modes require explicit audit. Preserve public host/path contracts where possible rather than assuming prefixed gateway URLs work with unchanged clients.
3. **Ingress coverage:** UI configuration, server-side rendering, internal URLs, webhooks, queue consumers, cron and maintenance scripts may bypass this Worker. The [UI service bases](../apps/ui3/src/data/catalyst/client.ts) and [SSR/public bases](../apps/sites/src/data/lib/catalyst/bases.ts) are independent configuration surfaces. Inspect deployed callers and Worker precedence before claiming all requests are routed.
4. **Data and freshness:** a path allowlist cannot establish complete listing/query scope. The gateway does not inspect generations, blocked heads or applied positions. Define dependencies, read-after-write behavior and stricter permission freshness before exposing target reads.
5. **Effects and sockets:** Social mutation sockets and internal Gatekeeper calls cannot be duplicated by forwarding HTTP ingress. Either adapt/capture compatible commands or retain their production owner; a job/internal call is a residual candidate only after replay assessment. Provider calls, live voice and queue acknowledgements need one owner.
6. **Exclusions:** shop, treasury, credits and marketplace remain out of scope. Generic gateway routing is not a financial firewall; do not configure these services for this migration.

## Required next gateway work

Rehearse the integrated favorite/vote flow and POI ordering before adding more routes. For each next command record exact public path/method, authentication, source/target body/response compatibility, generated IDs, retry identity, native target receipt and side effects. Test production rejection, lost responses after either commit, later deletes and ordering with any approved residual mutation.

No generic “send all requests twice” mode is approved. No newly added read-activation mode remains. The existing gateway/Places receipt changes are retained because they support native duplicate execution; see [PR rationale](repository-changes.md). Live DNS/route attachments, real signatures, Worker chains, session draining and readiness integration remain unverified.

## Additional service ingress

The gateway now has production-only Events/Notifications routing templates and a reviewed shadow-read path restriction. Notifications unsubscribe GETs mutate state and emit analytics, so they are rejected from shadow configuration. Eligible reads still need explicit sampling, target API/data parity and signature compatibility; the templates enable no shadow or replay traffic. See the [Notifications contract](notifications-gateway-contract.md).

## Current setter extension

The exact allowlist now covers favorites and tri-state likes. Optional `hybridOperationIds: "request-attempt"` supports unchanged clients for these setters; required caller IDs remain the default. See the [command protocol](places-hybrid-commands.md) for the distinction between accepted-attempt convergence and logical retry deduplication. No generic dual-write mode or public read-activation mode is added.
