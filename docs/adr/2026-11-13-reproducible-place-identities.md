# ADR: Reproducible place identities

- Status: Accepted
- Date: 2026-11-13

## Decision

Places and dcl-one generate the same UUIDv5 for a newly created place from the immutable creating deployment and its scope. Existing matched identities, including UUIDs preserved by the initial migration, retain their IDs. Request-operation IDs and content-rating record IDs are unrelated and keep their existing generation rules.

Use the standard UUID URL namespace and the UTF-8 JSON serialization of this string array, with no extra whitespace:

```json
["decentraland-place","v1","genesis","<deployment-entity-id>"]
```

For a world scene, replace `genesis` with `world:<lowercase-world-name>`. Preserve the entity ID's case and encoding; use the verified immutable entity ID, never a local database sequence or an independently calculated timestamp. Missing identity inputs must not fall back to random generation. Both implementations carry matching golden vectors.

Identity generation follows the lifecycle decision. Genesis preserves the matching base-parcel or equal-footprint identity; world scenes preserve a single overlapping active identity. When a new identity is required, the creating deployment determines its UUID. Reprocessing the same immutable deployment in the same scope can restore its deterministic retired identity and retain linked state; it must not steal an active identity belonging to a different accepted deployment. Hash equality is not authorization to accept a deployment.

## Why

Independent random UUIDs cannot match when both infrastructures process the same creation. A UUID based only on a parcel conflates different replacement decisions; a fresh deployment UUID on every update breaks stable references. Deriving only newly created identities from the creating deployment preserves continuity while removing randomness from the native operation.

This choice avoids adding a permanent gateway ID translator or a broad catalogue-row replication mechanism merely to carry random IDs. It does not alter the existing command-capture and replay protocol.

Genesis metadata-cache reads must find a retained place ID by its current deployment binding. A partial expression index on the deployment ID of content-derived catalogue rows supports those bounded per-page lookups without repeatedly scanning the full catalogue. It adds no new business data; index build and steady-state write costs still require staging measurement.

## Limits and rollout

Both infrastructures must start from equivalent identities and process equivalent accepted lifecycle transitions. Matching hash functions do not repair missed intermediate deployments, conflicting overlap state or different undeployment order. dcl-one's periodic Genesis projection can skip intermediate states; this change alone is not complete ordered deployment replay and does not widen the migration worker's immutable scopes.

Retired legacy bindings omitted by the initial import or cleared on retirement cannot be reconstructed from a hash. Preserve known existing IDs, but do not guess historical associations from a footprint. Full historical continuity would require a separate retained binding contract.

Run the additive schema migration on existing dcl-one databases to install the deployment-cache index; startup alone does not install a missing index. Roll out both implementations together before relying on deterministic identities for newly created places. Do not bulk-rekey existing catalogue, interaction or storage rows. Reconcile identities and validate ordered lifecycle coverage before public activation. Production remains authoritative during migration; source acceptance and runtime fencing remain required.

See the [migration entrypoint](../../sync-migration/README.md) for implemented sync scope and remaining activation gates.
