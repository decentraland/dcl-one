# Public Places identifiers for Worlds and Genesis scenes

## Decision

The production Worlds content server identifies a scene deployment by its content ID (CID). Places gives a scene a public UUID and can retain it through successive deployments. The Worlds mirror uses the public UUID when one Places row for the same world names the exact active deployment CID. World name, base parcel and parcel overlap alone cannot identify the indexed deployment.

The mirror commits scene content before looking up unresolved scenes in Places. Scheduled nodes can retry a miss after one minute, subject to worker capacity. Subsequent full passes also retry unchanged deployments and recover after a restart. On read-through-only nodes, a cached world's `/about` or `/scenes` request can trigger a background lookup. Concurrent reads share it and completed attempts have a one-minute cooldown. This lookup pairs the production Worlds server with production Places.

The catalogue mirror follows the public discovery feed. An opted-out scene may still have a Places UUID. The Worlds mirror therefore queries Places by the specific world name with `include_opted_out=true` when it has an unresolved WCS deployment. Explore owns the persisted identity and rechecks the deployment before recording a match.

## Local data and older links

A local place identity owns storage, grants and interactions. Discovery can publish the upstream UUID while the local UUID continues to resolve the same active scene. The binding records the source, world and deployment. If a later deployment keeps the internal identity but receives a different UUID upstream, a fresh match can activate the new public UUID. Former public UUIDs remain reserved to prevent reassignment to another scene.

Both services can index the same deployment at different times. Until a match is available, the node can serve the scene using its existing local identity. A lookup failure requires no transfer or deletion of stored data.

When a World's public UUID differs from the local identity and its exact deployment
proof expires, place detail returns 404 for that public UUID. Place status
reports a disabled entry and includes it in the count. The local identity
continues to describe its current deployment.

## Genesis City

A Genesis scene published directly to this node has a local Places UUID derived
from its base parcel. Production Places assigns its own UUID. The two catalogues
can receive the same deployment in either order.

Explore reconciles the two records when a complete walk of production Places
finds one eligible Genesis record for the local scene's base parcel and exact
active deployment CID. A match also requires the scene to be a live local
publication. A repeated UUID or an ambiguous match provides insufficient evidence.
The catalogue mirror and the local scene derivation both retry, so an existing
pair can converge without another scene deployment.

While the match holds, Places and Destinations publish one entry with the
production UUID. A link using the parcel-derived UUID opens that entry too.
Favorites and votes written through either link use the same local interaction
key. Scene storage continues to use the Genesis realm and base parcel.

If the local deployment changes before production Places indexes the new CID,
the local link describes the current local scene. The upstream record follows
the normal catalogue visibility rules and can describe an older deployment.
Sharing the same parcel alone never makes the two deployments equivalent.
