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

## New expansion work

Parallel work has been requested for missing Places catalogue coverage, event schedules and social settings. These are assessment/implementation workstreams, not accepted live coverage. Each must record its concrete contract, source writers, reasons for any external PR, migrations and tests before this table is expanded. See [rollout](rollout.md).
