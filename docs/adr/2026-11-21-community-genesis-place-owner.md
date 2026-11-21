# ADR: LAND ownership governs Genesis places in communities

- Status: Accepted
- Date: 2026-11-21

## Context

A community owner or moderator may add a place to a community, or remove one, only when the place belongs to them. The production service reads the owner of a Genesis place from the places catalogue, which takes it from the scene metadata. Scene metadata is written by whoever deploys the scene, so it can name any wallet, and it can stay unchanged after the LAND changes hands.

## Decision

The owner of a Genesis place is the owner of its base parcel in LAND state (`PlacesComponent::owns_places`, through the LAND authority or its configured fallback). Scene metadata never grants ownership of a Genesis place. A world place keeps the catalogue owner of the world.

- Adding places (REST, community create and edit, federated add) and removing a place run the same check.
- Only ownership counts: a LAND operator or a scene deployer who does not own the parcel cannot attach the place.
- When LAND ownership is unavailable, the check fails with 502 instead of falling back to metadata.

## Consequences

A wallet that production would accept, because the scene metadata names it, may be refused here, and the LAND owner may be accepted where production refuses. A transfer of the LAND moves the right to the new owner without a redeploy. The check does not revisit places already attached when the LAND changes hands.
