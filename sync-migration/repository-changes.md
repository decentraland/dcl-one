# Repository changes and PR rationale

Follow the [decision summary](README.md): duplicate compatible operations, then synchronize only demonstrated residual outcomes. Having a database or a job is not sufficient justification for a source-sync PR.

| Repository/change | Decision and reason |
| --- | --- |
| dcl-one standalone worker and private follower | Retain baseline, durable inbox/order, native favorite replay and atomic target receipts. They resolve lost responses and preserve operation ordering. Normal jobs/providers remain off. |
| Default source-only expansions | Remove likes/catalogue/schedule/settings row adapters, extra schema metadata and multi-source installation tooling. Those copied all scoped outcomes without first establishing command incompatibility. Keep pinned research and source-shaped favorite fixtures. |
| Places receipt handler, local `013d9ab` | Retain for native replay: after authentication, commit favorite operation, original response and outbox receipt together. This is why a source application change is needed; a gateway response alone cannot establish production commit. |
| Gateway production-receipt flow, local `ce3ad4a` | Retain exact favorite path allowlist, stable operation identity, reserved-header stripping and production response ownership. It does not issue speculative writes to both backends. |
| New gateway read-activation mode, local `cd5a6c5` | Reverted locally. It did not extend duplicate-command coverage and the public target read runtime is not available during sync. Existing routing and shadow comparisons remain. |
| Additional source services | No sync adapters approved yet. First document the exact non-replayable writer/result and why a native command path is insufficient. An adapter may capture evidence in a DB without copying business rows to the target. |

Relevant implementations: [worker](../services/migration-sync/README.md), [Places source command function](../services/migration-sync/source/places.sql), [target replay](../services/catalyst/explore/src/migration_replay.rs), [gateway audit](gateway-feasibility.md). The asset-bundle rewriter is outside this work and unchanged.

## PR status

Exact-branch GitHub lookup found no open or closed PR for `decentraland/dcl-one:feat/one-way-sync-service` or `decentraland/places:feat/hybrid-favorite-receipts`. Local removal does not imply any deployed change. Gateway GitLab MR status could not be verified because the configured token is expired; no MR was closed or remote branch changed. Older migration PRs are not evidence that these separate local branches were published.

The Places handler and existing gateway replay changes align with the selected design and are retained. Before opening or updating an external PR, state its operation contract, authentication, default-off behavior, required deployment order and rollback tests.

## Separate source defects discovered

- Events recurrence does not await a DB update: [evidence and implications](events-social-coverage-audit.md). A fix is about job completion/error handling, not adding table-wide sync.
- Notifications acknowledges queue input after failed persistence: [evidence and retry requirements](remaining-services-sync-audit.md). Neither gateway replay nor CDC can recover an event never durably recorded.
- Worlds owner updates omit the timestamp used by incremental import: [evidence](worlds-creator-sync-audit.md). A source timestamp fix alone does not solve deletes or make batch imports continuous replication.

These source defects remain pending; no source fix is claimed by this documentation cleanup.

## Cleanup verification

The retained worker passes 26 unit/CLI tests, two disposable PostgreSQL suites, four schema checks and Clippy. Tests verify unreceipted mutations leave target facts/checkpoints unchanged and cannot be overtaken by later accepted commands. Three soak-script tests pass. The gateway tree matches the retained pre-read-activation implementation and passes 129 tests, type checking and build. Local documentation links and ownership checks pass. No deployment was performed.
