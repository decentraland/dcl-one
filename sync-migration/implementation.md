# Implementation and validation

Recorded 2026-11-12. These are local implementation results, not production measurements. Commits are local and have not been pushed as part of this work.

## Current code

| Component | Local branch / commit | Implemented boundary |
| --- | --- | --- |
| dcl-one worker foundation | `feat/one-way-sync-service`, `f6062a8e6` | Separate removable process, durable inbox, ordered favorite application and independent package/image definition. |
| Integrated favorite pilot | Same branch, `33e464a4b` | Source outbox/bootstrap, command receipts, native replay, dedicated follower, reconciliation/recovery and metrics. |
| Interaction expansion | Same branch, `514ecc91e` | Source-only likes/dislikes with timestamps, independent versions, shared snapshot/order, adapter-v2 checks and additive upgrade. |
| Optional Places handler | `feat/hybrid-favorite-receipts`, `013d9ab` | Authenticated, opt-in atomic source favorite command/response receipts. Not required for source-only sync. |
| Optional gateway mode | `feat/hybrid-production-dispatch`, `ce3ad4a` | Exact public path allowlist, stable caller IDs and production response ownership. No best-effort target mutation. |

The [runbook](../services/migration-sync/README.md) documents installation, bootstrap, capture, apply, pause/resume, status, health, metrics, reconciliation and blocked-head retry. The [scope contract](../services/migration-sync/pilot-contract.json) declares the mapping and exclusions.

## Evidence at interaction expansion

- **28 worker unit/CLI tests** passed: envelope validation, source ordering, malformed input, dual/source-only separation, health and safe diagnostics.
- **Two PostgreSQL integration suites** passed. Capture tests cover snapshot continuity, concurrent transactions, rollback, vote flips, timestamps, deletes, mixed interaction transactions, counters, read-only polling, unsupported truncation, scope/generation drift and adapter mismatch. Seven isolated apply/upgrade scenarios cover atomicity, retries, HTTP uncertainty, native receipts, conflicts, like mappings and preserving v1 metadata through the target constraint upgrade.
- **Four schema checks** passed: complete schema rendering, object classification, additive ordered steps and layout identity.
- **Worker Clippy, formatting, ownership/protocol and diff checks** passed. The optional compiler cache failed locally; tests and Clippy succeeded with that cache disabled.
- **Follower evidence from the integrated pilot:** six PostgreSQL/router tests, one normal-runtime admission-guard test and full Catalyst compilation passed.
- **Places optional-handler evidence:** 11 tests, TypeScript checking and lint passed. Existing CORS supports the operation header for current signed-fetch requests.
- **Gateway evidence:** 129 tests, TypeScript checking and worker build passed. Review corrected the actual endpoint to `/api/places/{uuid}/favorites` and covered reserved-header stripping from both primary and shadow requests.
- **Repository CI-selection/regression evidence from the integrated pilot:** 58 checks passed, including a compiler-cache-related check rerun outside the sandbox.

Test sources: [capture](../services/migration-sync/tests/capture.rs), [ordered apply and HTTP faults](../services/migration-sync/tests/postgres.rs), [model](../services/migration-sync/src/model.rs), [follower](../services/catalyst/explore/src/migration_follower.rs).

## What this does not prove

No production deployment, source role installation, actual production traffic, multi-day soak, failover/restore rehearsal, live scheduled-job dataset or public read compatibility has been demonstrated. Nix package/image builds remain unverified locally. No production load, lag or retention budgets have been measured. Local fixture equivalence does not establish whole-service completeness.

The follower's private favorite read is not the public Places API. Normal runtime startup guards are not complete fencing against old binaries, manual writers or lost guard sessions. Capture assumes enabled triggers and reviewed source schemas; bypassing them invalidates continuity.

## Multi-source expansion

The bounded implementation now adds [Places catalogue](places-catalogue.md), [Events schedules](events-schedules.md) and [social settings](social-settings.md), with [independent source coordination](multiple-sources.md). These are source-only facts, not complete service migrations or public activation.

- **34 worker unit/CLI tests** passed.
- **Five PostgreSQL integration suites** passed: Places snapshot/capture, independent domains sharing one target, Events schedules, ordered application/recovery, and social settings. Coverage includes catalogue provenance and POI-category SQL updates, deletes, immutable scopes, source binding, absent settings, paused capture/application and oversized payload rejection.
- **Four schema checks, worker Clippy, formatting, ownership/protocol and document-link checks** passed.
- Reconciliation holds repeatable-read snapshots across size preflight and decode. No source application or client change is needed; each source DB requires capture installation and reviewed grants.

Tests: [shared domains](../services/migration-sync/tests/domains.rs), [Events](../services/migration-sync/tests/events_schedules.rs), [social settings](../services/migration-sync/tests/social_settings.rs), and the existing capture/apply suites linked above. Production schedulers, provider enforcement, future generated IDs and full service parity remain unverified or out of scope.

## Coverage audit and staging package

The [Places audit](places-coverage-audit.md) and [Events/Social audit](events-social-coverage-audit.md) pin reviewed sources and distinguish captured facts from missing workflows. The [installation package](staging-installation.md) and [rehearsal procedure](staging-rehearsal.md) provide the next executable steps; neither represents a live deployment.

The audit corrected a source compatibility defect: Places identifiers are character columns, not PostgreSQL UUID columns. Source scope joins and optional favorite SQL now compare text, tolerate unrelated world names and reject noncanonical scoped IDs instead of omitting them. The capture regression uses source-shaped character columns and a separate invalid-casing fixture.

Validation after the fix: **34 unit/CLI tests and all five PostgreSQL suites passed**. The soak script now routes all three sources and rejects arithmetic-overflow inputs; **three subprocess tests passed**, covering routing, invalid inputs and failure propagation. Clippy, formatting, ownership/protocol and local document-link checks passed. The Events recurrence unawaited-write finding remains a separate pending source fix, not a silently resolved item.

The disposable source-role test passed for all three adapters: non-owner writer CRUD, reader SELECT with denied mutation/function calls, and rejected missing grants, disabled triggers, non-inherited role membership, schema ownership and column-only UPDATE privileges. This validates the package on local PostgreSQL, not actual deployed roles.
