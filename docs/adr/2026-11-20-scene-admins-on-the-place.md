# ADR: Scene admin grants belong to the place

- Status: Accepted
- Date: 2026-11-20

## Context

`social.scene_admins` held the explicit admins of a scene under the id of the scene entity. A redeploy gives the scene a new entity id, so each redeploy lost the admins, their stream key access and their presenter control. The production service keeps the admins of a place across a redeploy.

## Decision

A grant is on the place of the scene, read from the active deployment by the room resolver (`ActiveDeployment::place_key`): the durable identity of a world scene, or the canonical base parcel of a Genesis scene. These are the keys that scene storage uses.

- A redeploy writes no row. A redeploy of the same place keeps every grant; a scene on a different place starts without admins.
- An owner change writes no row. Ownership is read when a request arrives.
- The cleanup of a removed place revokes the grants made before the removal: for a world place, those on the places of the world's active scenes; for a Genesis place, those on its base parcel when no enabled place has that base.
- A lookup of a catalogue place or of a whole world carries a key that no grant has, so a grant on one scene never decides for the world.
- The release script `schemas/releases/scene-admin-place-keys.sql` moves the grants of active deployments from the entity id to the place. An entity that is active in several places gives each place the grant. Rows of entities that are not deployed stay with their old key, and no request reads them.
- The move changes the key of live rows, so it is not a numbered step: an API from before the change reads and writes grants by entity id. Old replicas stop before `catalyst schema migrate`, the script runs after it, and `catalyst serve` refuses startup while a grant of an active deployment has the entity key.
- `social.scene_base_parcel` is the one SQL copy of the resolver's effective base parcel. The release script and the imports of production scene admins and scene bans call it.

## Consequences

The upgrade needs a maintenance window: stop the old API replicas, deploy, run `catalyst schema migrate`, run the release script, then start. A rollback restores the backup taken before the migrate. A stream key still ends at a redeploy ([decision](2026-11-20-stream-key-follows-authority.md)); the admins stay and can make the next key. `DATABASE.md` "Scene admin grants" has the workload and the invariants.
