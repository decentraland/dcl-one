# ADR: Removable hybrid migration coordinator

- Status: Accepted
- Date: 2026-11-12

## Decision

[The migration decision summary](../../sync-migration/README.md) defines the strategy: migrate before activation, duplicate compatible accepted operations, and implement production → dcl-one DB synchronization only for demonstrated residual outcomes. Classify operations and writers, not entire tables.

`services/migration-sync` remains a separately deployed and removable worker in the Rust workspace. Catalyst does not link or start it. Its independent deployment/retirement justifies a separate crate; metadata uses the existing shared schema bootstrap.

The retained bounded pilot imports a consistent favorite/vote/POI baseline and coordinates native favorite and vote commands. Source acceptance, mutation and original response commit together; target native mutation and receipt commit together. The coordinator verifies receipts before advancing ordered progress. Unreceipted source mutations are retained as evidence and block progress. The only post-baseline SQL fact mapping is the trusted source POI job's membership bit, justified by its mutable feed/catalogue dependencies and lack of an equivalent existing API request. It preserves unrelated categories. Unmarked POI writes block rather than being classified as job results.

The broader source-only likes/catalogue/schedule/settings adapters and their local-only metadata were removed because operation incompatibility had not been established. Their research informs future classification, not deployment. Never silently downgrade an existing test database or relabel capture generations.

Capture and replay run independently with separate credentials. Source-local outbox/receipts record evidence without contacting the target; they do not imply copying every business-row change. The minimal `catalyst migration-follower` process starts no ordinary jobs, mirrors or providers. Its private routes do not replace public APIs. Normal server startup remains fenced while migration streams exist; deployment-level fencing is still required for old binaries, lost sessions and manual writers.

## Ownership and retirement

Production owns user responses, business jobs, external effects and live sessions during coexistence. The target may reproduce only approved accepted operations. POI outcomes share ordering with native commands; any additional residual mapping must likewise prove that native results are not applied twice.

Before ownership transfer, fence source writers, account for in-flight commands/effects, reach final boundaries and reconcile. Independent target writes invalidate simple rollback to production; preservation of those writes needs a tested procedure. Archive receipts/history for the agreed recovery horizon before explicit retirement. Stopping the worker never deletes business facts or evidence.

See the [pilot runbook](../../services/migration-sync/README.md) for current commands and [operation backlog](../../sync-migration/operation-classification.md) for unresolved gates.
