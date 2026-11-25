# ADR: A stream key ends when its minter loses the place, and at a redeploy

- Status: Accepted
- Date: 2026-11-20

## Context

A scene stream key (`social.scene_stream_access`) admits an RTMP ingress, Cast streamers and a presentation bot to one scene room. The key recorded the wallet that generated it (`generated_by`), and nothing read that column again. A key worked until its expiry after its minter stopped being an admin, got a platform ban, lost a world permission or was no longer the owner. After a redeploy the scene has a new entity and a new room; the old key still published into the old room, where no scene runs.

## Decision

A key lives only while the wallet that generated it would still get it, for the scene that stands on the place of the key's room.

- The check uses the gates of the stream link: no platform ban, not on the deny list, and the key decision (owner or admin) on the scene deployed on the base parcel of the room's scene. For a world scene admin, the decision includes the wallet that gave the grant. Each world scene has its own key ([per-scene keys](2026-11-21-world-stream-keys-per-scene.md)). One function decides the ban and the deny list for the routes, for the minter of a key and for a watcher that joins.
- A Genesis scene that this node stores without an active deployment does not stand, so its key ends. A scene that this node does not store keeps its key.
- A key with no recorded minter (an imported one) is kept until its scene is replaced or the key expires.
- The check runs on each publisher token and each stream link, before `GET /scene-stream-access` answers, after `DELETE /scene-admin`, after a platform ban or an operator block, on a database notification ([stream key notifications](2026-11-20-stream-key-notifications.md)) and in a recheck job of its own (`comms-streaming-key-recheck`), apart from the job that expires keys.
- A key that fails the check is revoked like any other: one conditional update of the row, then the cleanup deletes the ingress, removes the publishers and tells the owner. The cleanup runs in a task of its own, so no request waits for LiveKit; a cleanup job tries a failed one again. The join guard refuses a publisher of a revoked key.
- The removal of a place ends its key in the removal's own transaction and sends no key notice.
- A source that cannot be read keeps the key and refuses the token.

## Alternatives

The pinned comms-gatekeeper never reads `generated_by`: a key lives until its expiry, and a removed admin keeps the stream. It keeps the key across a redeploy until the next stream link, so the old key publishes into a room with no scene.

## Consequences

This node is stricter than the pinned comms-gatekeeper on both rules. A streamer whose scene is deployed again asks for a new stream link; the next link is a key for the new room. An owner or admin that loses the place loses the stream at the next check, and the owner of the place gets the revocation notice. The routes, the key format and the room names do not change.
