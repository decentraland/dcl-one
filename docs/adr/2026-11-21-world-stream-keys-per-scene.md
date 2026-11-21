# ADR: A world scene has its own stream key

- Status: Accepted
- Date: 2026-11-21

## Context

Each world scene has its own LiveKit room (`worlds_service_scene_room`), and a stream key's ingress publishes only into the room it was minted for. The gatekeeper stored one key for a whole world, under the world's catalogue place. An admin of another scene of that world could read, reset or revoke a stream that never reached their scene. The pinned comms-gatekeeper keys the credential on the world scene's place (`getWorldScenePlace(worldName, position)`).

## Decision

A world scene's stream key (`social.scene_stream_access`) is stored under its place: `ActiveDeployment::place_key`, the `explore.world_scene_place` identity that scene admins and scene bans also use, kept across a redeploy of the same place. A Genesis scene's key stays under the catalogue place based on its base parcel.

- Every stream path resolves the scene first: `/scene-stream-access`, `/cast/generate-stream-link`, the Cast watcher, the presenter routes, the key recheck and its listener, the expiry job and the removal cleanup.
- A world scene's key decides on the scene's footprint. The world's owner and a deployment or streaming grantee whose grant has no parcel restriction pass on every scene. A grantee restricted to parcels passes on the scenes whose parcels overlap its grant, the overlap rule that `/scene-admin` and scene bans use.
- An explicit admin of the scene passes on that scene only, while the wallet that granted it holds world-wide authority (the owner or a grantee without a parcel restriction). An admin that a parcel-scoped grantee granted stays refused.
- A revocation in a world takes the signed scene when it is current, else the scene standing on the signed parcel now.
- A Cast watcher names a world scene by the body's `parcel`; without one, the world's only scene. A world with several scenes and no parcel answers 400, as upstream. The watcher link stays `/w/<world>`, so in a world with several scenes it needs the parcel.
- A removed world ends every key streaming into one of its scene rooms. A removed Genesis place ends its own key and never a world scene's key with the same id.
- A key stored under the world's catalogue place before this change stays valid as the key of the scene whose room it streams into. That scene reads, resets and revokes it, and a reset or renewal stores the replacement under the scene's place. No other scene reads it. Rows are not rewritten: publisher identities and presenter entries carry the place they were minted under. A legacy key ends at its first replacement, at its expiry (four days) or at a revocation.

## Alternatives

- One key for a whole world (the previous design): a key reaches one room, so the other scenes' admins managed a stream they could not see.
- A release step that rewrites each legacy row to its scene's place: a schema step runs before the object pass that creates `explore.world_scene_place` on an older database, and room names carry a length and escapes that SQL decodes poorly. A rewrite would also leave connected publishers under the old place in their identity, out of reach of the eviction of a later revocation.

## Consequences

Two scenes of one world have two keys, each in its own room. The routes, the key format and the room names do not change; `placeId` in a stream link is the scene's place. `GET /scene-stream-access` for a world scene with no key of its own answers 404 until a key is minted for it. This supersedes the per-world note of the [stream key authority decision](2026-11-20-stream-key-follows-authority.md).
