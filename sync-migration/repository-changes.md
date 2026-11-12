# Repository changes and PR rationale

**The current source-only rollout does not require a client change, the optional Places HTTP-handler PR, or hybrid gateway mode.** It does require the dcl-one implementation and an explicitly reviewed capture installation in the source database. None of the local commits below has been pushed or deployed as part of this work.

## Dependency matrix

| Repository or deployment | Source-only sync, current choice | Optional native favorite replay |
| --- | --- | --- |
| [dcl-one](https://github.com/decentraland/dcl-one) | Required: capture reader, baseline, transformations, ordered apply, reconciliation and isolated target operation. | Also provides coordinator, native target receipt endpoint and replay validation. |
| [Places source DB](https://github.com/decentraland/places/tree/2269acb/src/migrations) | Required installation: local triggers/outbox and explicit grants for every relevant writer. Runtime polling is read-only. | Also installs the atomic favorite command/response receipt function. |
| [Places application](https://github.com/decentraland/places/blob/2269acb/src/entities/UserFavorite/routes.ts) | No application deployment required. Keep `MIGRATION_CAPTURE_ENABLED` disabled. Existing handlers continue producing captured SQL changes. | Handler change required to call the atomic source function after authentication and bind the operation ID to the command. |
| [Cloudflare alb-router](https://dcl.tools/ops/cloudflare-workers/-/blob/487a8f2dfa9bba7e5d607cb3b291d7c38e0aead2/workers/alb-router/index.ts) | No new hybrid mode required. Existing upstream routing can remain. Row capture also covers writes bypassing the gateway. | Explicit path allowlist and operation-header handling are required for the planned gateway ingress. Production remains the response owner. |
| Clients | No changes. | Stable operation IDs and retry reuse are required by the current optional protocol. No client implementation has been completed here. |
| Other source services | No broad PRs authorized by a claim of generic coverage. Each mapping needs its own schema/writer/effect analysis. | Additional application work only where a particular native command needs atomic acceptance receipts or compatible replay semantics. |

## dcl-one: why these changes belong here

The [separate migration service](../services/migration-sync/README.md) owns transformation, ordering and temporary synchronization metadata. A batch loader cannot continuously detect deletes, bridge a source snapshot or distinguish a lost HTTP response from an uncommitted operation.

The current changes provide:

- [Capture and bootstrap](../services/migration-sync/src/capture.rs): snapshot-aligned export, read-only source polling, immutable generation/scope and reconciliation.
- [Mapping and application](../services/migration-sync/src/store.rs): atomic favorite/like facts, independent tombstones, counters and progress. Unsupported input blocks rather than being silently skipped.
- [Follower startup](../services/catalyst/src/migration_follower.rs): a target runtime that does not initialize ordinary jobs/providers, plus a startup admission guard for normal runtimes.
- [Native replay](../services/catalyst/explore/src/migration_replay.rs): optional target favorite mutation and receipt in the same transaction.
- [Operational controls](../services/migration-sync/src/ops.rs): health, lag metrics, explicit recovery and payload-free diagnostics.

Local branch: `feat/one-way-sync-service`. Foundation `f6062a8e6`, integrated favorite pilot `33e464a4b`, interaction expansion `514ecc91e`. The normal schema bootstrap includes worker state; the [adapter migration](../schemas/migrations/0039_migration_capture_adapter_v2.sql) widens allowed adapter identities without changing existing history.

## Places source database: why installation is required

The [source SQL](../services/migration-sync/source/places.sql) captures committed changes from `public.places`, `public.user_favorites` and `public.user_likes`, including internal or administrative writes. Triggers append to local outbox tables inside the same transaction; they never contact dcl-one. Without a verified logical CDC feed or this installation, the worker has no reliable stream of deletes and late commits.

This is a production database change even though no HTTP-handler deployment is needed. The [installation contract](../services/migration-sync/source/README.md) requires explicit source DDL permissions and grants to the actual application/writer roles. The functions run as the caller. Coordinate grants and installation before resuming writes; missing privileges can make triggered writes fail. Source lock contention, storage growth, failure recovery and trigger-bypass restrictions need staging evidence.

Adapter v3 includes catalogue metadata and likes/dislikes. Do not relabel or silently upgrade a v1/v2 stream; establish a reviewed new baseline that includes the additional history. Source polling needs only read permissions after installation.

## Places application PR: why it exists, and when it is needed

Local branch: `feat/hybrid-favorite-receipts`, commit `013d9ab`. This PR is **optional for the current source-only rollout**. It should not be described as a prerequisite for ordinary database synchronization.

Its purpose is to exercise dcl-one's native favorite implementation safely after production has accepted the same logical command. The gateway cannot infer production commit status from an HTTP response or timeout. The proposed handler therefore:

1. Performs existing source authentication before the migration branch.
2. Recognizes an opt-in `x-dcl-operation-id`, behind `MIGRATION_CAPTURE_ENABLED=true`.
3. Calls a parameterized SQL function that commits the favorite mutation, derived source count, original response and command outbox metadata together.
4. Binds the operation to the authenticated canonical wallet, place and favorite value. Activity score is excluded from the semantic fingerprint because it may change between retries.
5. Returns the stored response for an identical retry, including after later changes remove the place. Conflicting identity reuse fails; database errors do not fall back to an unreceipted write.

Requests without the header keep the existing handler. Its writes are still captured by database triggers, but they use transformed application rather than native duplicate execution. The optional handler must not turn an unverified gateway header into authentication.

The public endpoint is `PATCH /api/places/{uuid}/favorites`: the [server mounts the router under `/api`](https://github.com/decentraland/places/blob/2269acb/src/server.ts). The same server's existing `/api` CORS middleware allows the operation header for current signed-fetch requests; no CORS change was needed. Reusing an expired client signature is not part of delayed target replay: the target receives an authenticated pointer to already committed source evidence.

Validation: 11 handler/helper tests, type checking and lint passed. Real SQL receipt behavior is covered by dcl-one's disposable PostgreSQL suites. No source HTTP-handler changes are needed for the new likes/dislikes source-only mapping.

## Gateway PR: why it exists, and when it is needed

Local branch: `feat/hybrid-production-dispatch`, commit `ce3ad4a`. This is also **optional for source-only sync**. The intended worker is `workers/alb-router`; the asset-bundle rewriter is outside this work and must remain unchanged.

The optional mode forwards exact allowlisted favorite requests to production with a stable caller operation ID. It does not send a best-effort mutation to dcl-one. The source transaction/outbox provides durable delivery, and the coordinator replays later. This avoids losing the second request when a worker background task ends and avoids executing a target command rejected by production.

The mode requires the actual `/api/places/{uuid}/favorites` path and rejects signed pathname rewrites and the unverified short alias. Reserved operation headers are stripped outside the approved path/method, including shadow reads. Production errors never trigger target write failover. Ordinary methods and unrelated routes continue through production.

Validation: 129 tests, TypeScript checking and worker build passed. Deployed Worker precedence, host aliases, allowlists and routing configuration still need staging verification. Existing read shadowing is not durable mutation synchronization.

## Requirements for PRs into additional services

Before opening another source-service PR, add a subsection here identifying:

- The exact dataset, source writer and target invariant it enables.
- Whether it needs database capture only, an application receipt, an effect-ownership control, a protocol adapter or a missing target mapping.
- Why existing code or row capture cannot satisfy that requirement alone.
- Authentication, transaction boundaries, generated IDs, deletes, callbacks and external effects affected.
- Default/disabled behavior, compatibility, deployment order, rollback and acceptance tests.

Do not add synchronization code to every application write by default. For a pure fact mapping, database capture can cover HTTP, jobs and internal writers. Application changes are justified when required behavior is not represented by copied rows—for example an authorization revocation callback or explicit notification delivery—or when a selected command needs a durable native replay contract. Shop and credits remain excluded; discovery of a dependency is not permission to modify them.

## Newly included source-only mappings

- **Places catalogue:** extend the source database adapter to v3 so existing row writers,
  including the reviewed daily POI category job SQL, enter the same ordered stream as
  interactions. No Places handler or client PR is required. Native moderation ownership
  conflicts block; target cleanup intents remain inert. [Full rationale](places-catalogue.md).
- **Events schedules:** install a local capture adapter for selected `schedule` UUIDs;
  the dcl-one mapping handles rows/deletes. No Events application or client PR is needed.
  Random future schedule IDs and event workflows are not implicitly included.
  [Full rationale](events-schedules.md).
- **Social settings:** install local capture for selected wallets and map all three
  setting values plus row absence. No social-service handler or client PR is needed.
  Source Gatekeeper effects are not replayed and target privacy authority stays off.
  [Full rationale](social-settings.md).

The [shared coordinator](../services/migration-sync/src/domain_capture.rs) adds separate
source identities, scopes and positions. These are dcl-one changes; source installations
still require explicit database grants and operational review even without application PRs.

## Staging audit follow-up

The [installation package](staging-installation.md) supplies explicit source writer/reader grants and preflight checks. It requires a controlled source write fence while installing triggers and grants; this is an operational database change, not an application sync hook.

The [Events/Social audit](events-social-coverage-audit.md) identifies a separate Events recurrence bug: its job does not await a database update. A focused Events PR should await completion and test error propagation before relying on job completion as rehearsal evidence. That fix is pending; it is not required to capture successfully committed schedule rows. The [Places audit](places-coverage-audit.md) records the source ID-type compatibility fix in dcl-one and the remaining world/catalogue dependencies.
