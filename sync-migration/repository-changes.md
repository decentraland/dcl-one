# Repository changes and PR rationale

Follow the [decision summary](README.md): duplicate compatible operations, then synchronize only demonstrated residual outcomes. Having a database or a job is not sufficient justification for a source-sync PR.

| Repository/change | Decision and reason |
| --- | --- |
| dcl-one standalone worker and private follower | Retain baseline, durable inbox/order, native favorite/vote replay and atomic target receipts. They resolve lost responses and preserve operation ordering. Normal jobs/providers remain off. |
| Default source-only expansions | Remove likes/catalogue/schedule/settings row adapters, extra schema metadata and multi-source installation tooling. Those copied all scoped outcomes without first establishing command incompatibility. Keep pinned research and source-shaped favorite fixtures. |
| Places handlers and POI job, local `f5ee87c` | Native favorite/vote handlers commit accepted command, original response and outbox receipt together. The POI job records only its approved membership outcome. This is why a source application change is needed; a gateway response alone cannot establish production commit. |
| Gateway production-receipt flow, local `25e4193` | Exact favorite/vote path allowlist, stable or opt-in per-attempt identity, reserved-header stripping and production response ownership. It does not issue speculative writes to both backends. |
| New gateway read-activation mode, local `cd5a6c5` | Reverted locally. It did not extend duplicate-command coverage and the public target read runtime is not available during sync. Existing routing and shadow comparisons remain. |
| Additional source services | Only the Places POI membership outcome is approved as residual. Additional writers still need evidence that compatible command replay is insufficient. An adapter may capture evidence in a DB without copying business rows to the target. |

Relevant implementations: [worker](../services/migration-sync/README.md), [Places source command function](../services/migration-sync/source/places.sql), [target replay](../services/catalyst/explore/src/migration_replay.rs), [gateway audit](gateway-feasibility.md). The asset-bundle rewriter is outside this work and unchanged.

## PR status

| Review | Scope/status |
| --- | --- |
| [dcl-one #195](https://github.com/decentraland/dcl-one/pull/195) | Existing initial migration PR; open. |
| [dcl-one #706](https://github.com/decentraland/dcl-one/pull/706) | Draft hybrid worker/follower, stacked on #195. Retarget to main after that dependency merges. |
| [Places #901](https://github.com/decentraland/places/pull/901) | Draft native favorite/vote receipts and selective POI producer. |
| [Events #1032](https://github.com/decentraland/events/pull/1032) | Recurrence job completion fix; no Events sync installed. |
| [Gateway !446](https://dcl.tools/ops/cloudflare-workers/-/merge_requests/446) | Draft alb-router routing, safe shadow comparison and Places command ingress. Created through SSH push options because the GitLab API token is expired. |

Branches are published for review; nothing is deployed. The dcl-one branch was rebased onto the initial migration branch to exclude unrelated upstream commits; service, schema and sync-document trees remained unchanged. Source flags and route activation require matching protocol installation and rehearsal evidence.

## Separate source defects discovered

- Events recurrence completion is fixed locally on refreshed upstream `master`: [endpoint contract, evidence and validation](events-command-contract.md). It awaits persistence and propagates failure; this does not install Events synchronization.
- Notifications acknowledges queue input after failed persistence: [evidence and retry requirements](remaining-services-sync-audit.md). Neither gateway replay nor CDC can recover an event never durably recorded.
- Worlds owner updates omit the timestamp used by incremental import: [evidence](worlds-creator-sync-audit.md). A source timestamp fix alone does not solve deletes or make batch imports continuous replication.

Notifications and Worlds fixes remain pending. The Events recurrence fix is local and not deployed.

## Native vote and selective POI increment

The current extension adds native vote commands to the source handler, gateway allowlist and target receipt protocol. The [command contract](places-hybrid-commands.md) explains stable or per-attempt identities. Source handler changes are needed for atomic accepted-command evidence, not row replication.

The [POI job contract](places-poi-residual.md) justifies one selective outcome: its effective POI membership bit. The source job requires an explicit default-off path to commit that result with trusted producer identity. Unknown POI writers are observed but block rather than being mistaken for approved job results. A failed external POI fetch now stops the job instead of being treated as an authoritative empty list. These changes leave unrelated categories and all other catalogue fields outside the mapper.

## Current local verification

The gateway passes 301 tests, TypeScript checking and build. Places passes 42 focused tests plus TypeScript and scoped lint checks; its commit hooks also passed related tests, formatting and type checks. The worker passes 27 unit/CLI tests and three real PostgreSQL integration suites. They cover source acceptance/native replay, repeated attempt identities, tampered inbox rejection, POI-only mapping, rollback, reconciliation and blocked-head ordering. Clippy with warnings denied, four schema checks, ownership generation and local documentation links also pass. No production deployment or multi-day soak is claimed.

## Events follow-up and scope validation

The [Events endpoint contract](events-command-contract.md) selects schedule PATCH before creation and RSVP. No Events gateway route or row adapter is enabled. The source change has a job-lifecycle purpose independent of replication.

The worker now validates entity IDs from vote commands and POI results even when `changes` is empty. It rechecks the same scope before application. PostgreSQL regressions reject out-of-scope no-ops on enqueue and block directly inserted retained payloads without advancing the checkpoint. The full 27 unit/CLI tests, three PostgreSQL suites and Clippy pass. Events has six focused recurrence tests plus passing type/lint checks.
