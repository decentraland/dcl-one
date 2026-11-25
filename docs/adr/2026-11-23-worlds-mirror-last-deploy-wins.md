# Worlds mirror: last deploy wins

- Status: Accepted
- Date: 2026-11-23
- Decided by: user, 2026-11-24, in the review of this change: place identities
  keep the overlap rule unchanged on a takeover, and a takeover that keeps a
  place id queues no place removal.

## Context

A node that mirrors an upstream worlds server also accepts publishes of the same
world names, and both servers are authoritative for those names. The mirror refused
every world holding a locally published scene. So once a world had been published
here, a later deployment of that name upstream never showed on this node.

A local publish already takes a mirrored world: it is refused only when a scene it
overlaps is newer, and it then retires every mirrored scene of the world, overlapping
or not, and marks the world row local. A world holds scenes of one source at a time.

## Decision

The scheduled mirror pass applies the same rule in the other direction: the last
deployment to a world name wins, with the whole world compared.

- **Comparison.** The upstream's newest scene (from its `/scenes` listing) against the
  newest local scene of the world. A deployment counts at its entity timestamp, never
  later than its own write: the local row's update time, and upstream the listed
  deploy time (`updatedAt`, else `createdAt`) and now. Without a numeric timestamp, a
  local scene counts at its row's update time and an upstream scene at its listed
  deploy time. An upstream scene listed without a deploy time never replaces a local
  one, whatever its entity timestamp.
- **Guards.** A local world stays local while it is blocked here or a publish of it
  is queued or being sent upstream (`worlds.forward_outbox`), so a forward is never
  dropped and an operator block is never lifted by the mirror.
- **Ties.** Equal times go to the byte-wise greater entity id, as publishes order
  them. The same entity on both sides (a publish this node forwarded, a row the
  worlds import wrote as local) keeps the world local.
- **Newer upstream.** In the transaction that writes the upstream's scenes, under the
  world row's lock, every local scene of the world is retired into
  `worlds.retired_world_scenes`, as a replacing publish retires scenes. A local scene
  that the upstream lists by the same id becomes the mirrored row. The world row
  is marked mirrored. Place identities follow the overlap rule: an upstream scene
  overlapping exactly one retired scene keeps that identity, and so the storage
  scoped to it. The rule is the same as for a publish here: it compares
  footprints only, not who deployed either scene. The access setting and world settings are copied as for any mirrored
  world, unless the owner set them here and still holds the name. An operator block
  stands.
- **Local not outdated.** The world is refused as before, and only the scenes the
  mirror wrote there are retired. The listing's timestamps decide this before
  anything is downloaded. The write decides again on the hash-checked entities, so a
  publish that lands during the download is compared too.
- **Read-through fills** never replace a local publish.
- This path writes no local activity record.
- **Place removal.** The explore catalogue then prunes the world's local row. That is
  not a place removal while the world is mirrored and one of its scene places is
  active, so no removal is queued and the world's bans and scene admin grants stay.

## Consequences

- A world published here and then deployed again upstream shows the upstream
  scenes after the next pass. A later local publish takes the world back.
- The comparison is per world, not per parcel. If the upstream's older scenes
  stand on parcels a local scene took later, those older scenes return together
  with the upstream's newest scene.
- A local publish whose entity timestamp is older than a mirrored scene it does
  not overlap is accepted, then replaced on the next pass, because that mirrored
  scene is the newest deployment to the name.
- A pass row carries the upstream's chain only when it checks. A local publish
  can be replaced by an upstream scene whose signer this node never verified,
  on the upstream's word as for any mirrored world.
- A local identity that a takeover does not keep is retired as a replacing publish
  retires one: its storage stays, and its grants no longer resolve to a scene.
- Every pass reads the `/about` and `/scenes` of each listed world that this node
  keeps local. Entities and files are fetched only when the upstream is newer.
