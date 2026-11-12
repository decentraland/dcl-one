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

Exact-branch GitHub lookup found no open or closed PR for `decentraland/dcl-one:feat/one-way-sync-service` or `decentraland/places:feat/hybrid-favorite-receipts`. Local removal does not imply any deployed change. Gateway GitLab MR status could not be verified because the configured token is expired; no MR was closed or remote branch changed. Older migration PRs are not evidence that these separate local branches were published.

The Places handler and existing gateway replay changes align with the selected design and are retained. Before opening or updating an external PR, state its operation contract, authentication, default-off behavior, required deployment order and rollback tests.

## Separate source defects discovered

- Events recurrence does not await a DB update: [evidence and implications](events-social-coverage-audit.md). A fix is about job completion/error handling, not adding table-wide sync.
- Notifications acknowledges queue input after failed persistence: [evidence and retry requirements](remaining-services-sync-audit.md). Neither gateway replay nor CDC can recover an event never durably recorded.
- Worlds owner updates omit the timestamp used by incremental import: [evidence](worlds-creator-sync-audit.md). A source timestamp fix alone does not solve deletes or make batch imports continuous replication.

These source defects remain pending; no source fix is claimed by this documentation cleanup.

## Native vote and selective POI increment

The current extension adds native vote commands to the source handler, gateway allowlist and target receipt protocol. The [command contract](places-hybrid-commands.md) explains stable or per-attempt identities. Source handler changes are needed for atomic accepted-command evidence, not row replication.

The [POI job contract](places-poi-residual.md) justifies one selective outcome: its effective POI membership bit. The source job requires an explicit default-off path to commit that result with trusted producer identity. Unknown POI writers are observed but block rather than being mistaken for approved job results. A failed external POI fetch now stops the job instead of being treated as an authoritative empty list. These changes leave unrelated categories and all other catalogue fields outside the mapper.

## Current local verification

The gateway passes 301 tests, TypeScript checking and build. Places passes 42 focused tests plus TypeScript and scoped lint checks; its commit hooks also passed related tests, formatting and type checks. The worker passes 27 unit/CLI tests and three real PostgreSQL integration suites. They cover source acceptance/native replay, repeated attempt identities, tampered inbox rejection, POI-only mapping, rollback, reconciliation and blocked-head ordering. Clippy with warnings denied, four schema checks, ownership generation and local documentation links also pass. No production deployment or multi-day soak is claimed.
