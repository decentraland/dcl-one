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

## Place UUIDs: baseline coverage and ongoing creation gap

Checked against refreshed [Places master at 2269acb7](https://github.com/decentraland/places/tree/2269acb77d591371d21e55e80aaf26b8e70733d7) on 2026-11-13. This is source inspection, not a deployed creation/redeployment test.

Places [generates a random UUID for a new place](https://github.com/decentraland/places/blob/2269acb77d591371d21e55e80aaf26b8e70733d7/src/entities/CheckScenes/task/processContentEntityScene.ts#L217). Existing matched places retain their ID because their data overrides the generated default. For world scenes, [exactly one overlapping place retains its identity; zero or multiple overlaps create a new one](https://github.com/decentraland/places/blob/2269acb77d591371d21e55e80aaf26b8e70733d7/src/entities/CheckScenes/task/resolveWorldDeployment.ts#L138). A place UUID is distinct from the immutable deployment/entity ID.

The [baseline import](https://github.com/decentraland/dcl-one/blob/feat/one-way-sync-service/tooling/migration/sources/places/MAPPING.md) preserves source catalogue IDs and world-scene UUIDs. That permits favorite/vote replay against the same existing identity within the approved scope. The gateway's operation UUID identifies a request, not a place, and cannot solve resource identity differences.

Independent deployment processing does not preserve new IDs: the target [Genesis derivation](https://github.com/decentraland/dcl-one/blob/feat/one-way-sync-service/services/catalyst/explore/src/places/derive.rs#L169) uses a UUIDv5 derived from the base parcel, while its [world-scene assignment](https://github.com/decentraland/dcl-one/blob/feat/one-way-sync-service/services/catalyst/explore/src/places/scene_identity.rs#L64) generates UUIDv4 when a new identity is needed. Neither reproduces a newly generated production UUID. Duplicating the deployment request alone is insufficient.

**Current gap:** the pilot has an immutable allowlist and no ongoing catalogue-identity capture or mapping lane. New IDs are outside its coverage; a replacement can also retire an old scoped ID. Do not treat the initial migration or request replay as complete coverage for new places, splits, merges or redeployments. World-scene identity divergence also affects player-storage namespace bindings; the player pilot deliberately stops when its fixed binding changes.

**Recommended extension:** production remains the identity authority. Capture its committed creation/replacement decision, including the chosen UUID, deployment identity, footprint and retired IDs. Apply that identity transition before dependent commands, with duplicate/conflict checks and atomic progress. Prefer a native target operation accepting the trusted source-assigned identity; use a narrow residual identity projection only where native replay cannot represent the decision. This does not justify copying favorites/likes rows. Avoid rewriting public IDs at the gateway or changing UUID generation on both sides without a shared identity/lifecycle contract.

Before expanding the pilot, test new Genesis/world scenes, ID-preserving redeployments, footprint changes, splits/merges, delayed dependent writes and replay after restart. These cases are not yet implemented by the migration worker.
