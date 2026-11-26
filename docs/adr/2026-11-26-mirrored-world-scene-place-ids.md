# Public Places identifiers for mirrored world scenes

## Decision

The production Worlds content server identifies a scene deployment by its content ID (CID). Places gives a scene a public UUID and can retain it through successive deployments. The Worlds mirror uses the public UUID when one Places row for the same world names the exact active deployment CID. World name, base parcel and parcel overlap alone cannot identify the indexed deployment.

The mirror reads the paginated Places result for each world with unresolved scenes. It commits scene content independently. A later scheduled pass retries unresolved scenes even when their content has not changed. With only read-through enabled, a read of a held world's `/about` or `/scenes` can start a background lookup. Concurrent reads share the lookup and a completed attempt has a one-minute cooldown. This lookup pairs the production Worlds server with production Places.

## Local data and older links

A local place identity owns storage, grants and interactions. Discovery can publish the upstream UUID while the local UUID continues to resolve the same active scene. The binding records the source, world and deployment. If a later deployment keeps the internal identity but receives a different UUID upstream, a fresh match can activate the new public UUID. Former public UUIDs remain reserved to prevent reassignment to another scene.

Both services can index the same deployment at different times. Until a match is available, the node can serve the scene using its existing local identity. A lookup failure requires no transfer or deletion of stored data.
