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
