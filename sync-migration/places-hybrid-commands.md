# Places hybrid commands

The [decision summary](README.md) selects native duplicate execution before residual DB sync. This pilot supports exact scoped Genesis paths:

- `PATCH /api/places/{UUID}/favorites`, boolean `favorites`.
- `PATCH /api/places/{UUID}/likes`, `like: true`, `false`, or `null` (withdrawal). Omitted `like` retains the source default `true`.

No world-name paths, generic creates, financial commands or external provider effects enter this protocol.

## Execution and retries

The gateway sends one request to production and returns its response. After authentication and schema validation, the opted-in source handler calls a transactional function that records the native source change, original response and accepted command together. The worker captures that evidence; a dedicated target listener executes the target's native interaction helper and commits its receipt atomically. The worker verifies the receipt and resulting state before advancing. This is two native executions with durable delivery, not a row copier or two speculative HTTP calls.

Source-computed activity and timestamps travel as deterministic command inputs. Same-state setters preserve existing activity/timestamps; a vote flip keeps creation time and changes update time; `null` removes the vote. The target recomputes counts from native facts. Commands and the [POI residual](places-poi-residual.md) share one ordered source stream, so neither can overtake a blocked outcome.

Operation identity is scoped to the authenticated actor, entity, command kind and semantic value. Reusing an ID with a different favorite/vote command is rejected. Stored responses resolve retries after an ambiguous response, including after later entity changes. Neither a claimed HTTP success nor a timeout proves target execution; only its committed receipt does.

## Gateway identity policies

`hybridOperationIds: "required"` is the default: clients provide a canonical `x-dcl-operation-id`, reused for retries of the same logical command.

`hybridOperationIds: "request-attempt"` is an explicit alternative for these setters only. When the header is absent, the gateway generates an ID for that request attempt. Valid caller IDs are retained; malformed caller IDs still fail. ID generation failure stops before dispatch. Nothing is generated for other methods, routes, world names or non-hybrid traffic.

A retry after a lost production response gets a new attempt ID. It is another real source operation. If the state is unchanged it is a native no-op; if an opposite command intervened, production decides the new state and dcl-one follows the same acceptance order. This guarantees neither logical client-intent deduplication nor exactly-once HTTP delivery. It is unsuitable for generated resources, payments and non-idempotent side effects.

## Required repository changes

- **Gateway:** extend the exact setter allowlist and optional attempt identity policy; preserve bodies, signed pathnames and production ownership. No read-activation mode, direct target write, automatic retry, or deployment configuration change is included.
- **Places:** the `MIGRATION_CAPTURE_ENABLED` path records acceptance and response atomically. Without it, the gateway cannot know whether a lost response committed. The flag remains off by default; errors in the opted-in path never fall back to unreceipted mutation.
- **dcl-one:** native vote replay joins favorite replay, with strict command/evidence validation, scoped baseline, shared ordering and result verification. There is no continuous favorite/vote row application fallback.

The new source adapter identity is `places_hybrid_commands_v1` with source control version 4. It does not reinterpret the older removed v2/v3 capture formats. Install with a reviewed new consistent baseline; do not reset existing evidence or relabel a cursor. Initial baseline migration may import favorite/vote facts and the POI bit; that is distinct from continuously copying compatible command outcomes.

## Remaining limits

The isolated follower still has private migration endpoints, not public Places APIs. Normal target jobs/providers stay off. Actual ingress, source role grants, deployed schema, source lock contention, retention and restore behavior require staging verification. Out-of-scope or unreceipted changes block instead of being silently guessed.

The opt-in vote handler currently resolves voting power before the source receipt call for non-null votes. Stored no-op/retry results remain unchanged, but such requests can wait on the Snapshot provider even when an ordinary same-state vote would avoid it. Measure this bounded latency difference before activation; a source receipt shortcut must preserve atomicity and must not use a stale read to omit activity for a real mutation.

## Place UUIDs: reproducible creation and lifecycle coverage

Implemented in [Places #902](https://github.com/decentraland/places/pull/902) and [dcl-one #706](https://github.com/decentraland/dcl-one/pull/706). The [reproducible identity decision](../docs/adr/2026-11-13-reproducible-place-identities.md) selects UUIDv5 for new Places identities in both infrastructures. Existing matched/imported UUIDs remain unchanged. The gateway's operation UUID identifies a request, not a place.

The shared input is the compact UTF-8 JSON array `["decentraland-place","v1",scope,entityId]`, under the standard UUID URL namespace. Scope is `genesis` or `world:<lowercase-world-name>`; entity ID is the immutable creating deployment ID, case-preserved. No clocks, local sequences or random fallback participate. Identical inputs produce identical IDs in TypeScript and Rust.

Generation happens only when the lifecycle rules require a new identity. Genesis preserves a matching base or equal parcel footprint; world scenes preserve exactly one active overlap. Ordinary updates keep their existing UUID even though their deployment changes. Deterministic reactivation of the same creating entity may retain earlier linked state; a conflicting active binding must not be overwritten.

### Why Places needs a source change

The [production generator](https://github.com/decentraland/places/blob/2269acb77d591371d21e55e80aaf26b8e70733d7/src/entities/CheckScenes/task/processContentEntityScene.ts#L217) uses random UUIDs for new places. Changing only dcl-one cannot reproduce them. Both services must adopt the same new-identity contract; no client or gateway ID rewriting is required. Rating record IDs and request-operation IDs are outside this change.

The [baseline import](https://github.com/decentraland/dcl-one/blob/feat/one-way-sync-service/tooling/migration/sources/places/MAPPING.md) preserves existing catalogue IDs and world-scene UUIDs. Do not bulk-rekey these rows or their favorites, likes and player values.

### What this does not synchronize

Reproducible generation does not supply missing deployment/undeployment events or reproduce a different prior overlap state. The services must start with equivalent identities and process equivalent accepted transitions. dcl-one's periodic Genesis projection can skip intermediate deployments; full ordered lifecycle replay remains an activation requirement. The current migration pilot's scopes stay immutable, and no generic catalogue synchronization is enabled.

Retired legacy bindings omitted by import or cleared on retirement cannot be recovered from a UUID hash. No footprint-based historical guessing is allowed. Broader historical continuity needs a separate binding-history contract. Player storage also remains restricted to its fixed approved scene bindings.

Before public activation, verify new Genesis/world scenes, preserved legacy IDs, footprint changes, splits/merges, delayed dependent writes, reactivation and restart recovery through the actual deployment pipeline. Matching hash vectors alone are not proof of lifecycle convergence. If lifecycle decisions cannot be replayed equivalently, capture the source decision narrowly; do not add favorite/like row copying as a workaround.

### Validation (2026-11-13)

Both implementations use the same four golden vectors, independently checked with Python UUIDv5. Places passed 734 unit tests across 55 suites and the complete 531-test integration suite across 28 suites, with no skips, using a freshly migrated PostgreSQL database and CI's LocalStack 4.14.0. Source/test TypeScript checks and focused ESLint passed. dcl-one passed 177 Places tests (one existing ignored), 168 storage tests and the migration worker's 29 unit/CLI tests plus all five PostgreSQL integration suites. Clippy, four schema checks and the existing-database schema-upgrade regression passed.

Coverage includes imported UUID preservation, same-base/equal-footprint updates, same-page splits/merges, stale or conflicting active bindings, deterministic retired reactivation, retained counters/curation and source moderation conflicts. No production rollout, multi-day lifecycle convergence or staging performance claim is made. Existing dcl-one databases must run the additive schema migration to install the deployment-cache index.

The full integration run caught fixtures that reused one entity ID for different immutable scene contents. Those fixtures now hash their serialized scene bytes, while explicit IDs remain available for ordering scenarios. Accepted same-entity reactivation and duplicate delivery retain one identity and curation. Production identity-conflict guards remain unchanged.
