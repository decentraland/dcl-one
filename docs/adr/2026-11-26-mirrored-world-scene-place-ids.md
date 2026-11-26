# Public Places identifiers for mirrored world scenes

## Decision

The production Worlds content server identifies a scene deployment by its content ID (CID). Places gives a scene a public UUID and can retain it through successive deployments. The Worlds mirror uses the public UUID when one Places row for the same world names the exact active deployment CID. World name, base parcel and parcel overlap alone cannot identify the indexed deployment.

The mirror commits scene content before looking up unresolved scenes in Places. Scheduled nodes can retry a miss after one minute, subject to worker capacity. Subsequent full passes also retry unchanged deployments and recover after a restart. On read-through-only nodes, a cached world's `/about` or `/scenes` request can trigger a background lookup. Concurrent reads share it and completed attempts have a one-minute cooldown. This lookup pairs the production Worlds server with production Places.

The catalogue mirror follows the public discovery feed. An opted-out scene may still have a Places UUID. The Worlds mirror therefore queries Places by the specific world name with `include_opted_out=true` when it has an unresolved WCS deployment. Explore owns the persisted identity and rechecks the deployment before recording a match.

## Local data and older links

A local place identity owns storage, grants and interactions. Discovery can publish the upstream UUID while the local UUID continues to resolve the same active scene. The binding records the source, world and deployment. If a later deployment keeps the internal identity but receives a different UUID upstream, a fresh match can activate the new public UUID. Former public UUIDs remain reserved to prevent reassignment to another scene.

Both services can index the same deployment at different times. Until a match is available, the node can serve the scene using its existing local identity. A lookup failure requires no transfer or deletion of stored data.

When a public UUID differs from the local identity and its exact deployment
proof expires, place detail returns 404 for that public UUID. Place status
reports a disabled entry and includes it in the count. The local identity
continues to describe its current deployment.
