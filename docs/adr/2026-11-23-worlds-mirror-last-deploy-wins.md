# Worlds mirror: last deploy wins

- Status: Accepted
- Date: 2026-11-23
- Decided by: user, 2026-11-24, in the review of this change: on a takeover an
  upstream scene keeps a local scene's place identity only when it covers that
  scene's whole footprint, and a takeover that keeps a place id queues no place
  removal.

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
- **Ties.** Equal times go to the byte-wise greater entity id. A local scene
  identical to the upstream's newest keeps the world local unless it has an
  accepted forward to that source (below).
- **Newer upstream.** In the transaction that writes the upstream's scenes, under the
  world row's lock, every local scene of the world is retired into
  `worlds.retired_world_scenes`, as a replacing publish retires scenes. A local scene
  that the upstream lists by the same id becomes the mirrored row. The world row
  is marked mirrored. Place identities follow the overlap rule with one more
  condition: an upstream scene keeps a retired local scene's identity, and so the
  storage scoped to it, only when it overlaps exactly that one identity and its
  parcels cover that scene's whole footprint. Otherwise it gets a new identity and
  the local one is deactivated, as on any retirement; storage rows are never
  deleted. An upstream scene that covers one local scene and stands on part of
  another overlaps two, so it gets a new identity and both retire. This matches
  the upstream deploy check (`decentraland/worlds-content-server` at
  `04c8eea2ab96d35703ceae76140106da821ee409`, `src/logic/validations/scene.ts`):
  a deployer who is not the name's owner needs permission over the new parcels
  and the whole footprint of every scene the deployment replaces. Upstream checks
  the signer's permissions; this node compares footprints only, not who deployed
  either scene. A publish here and a pass that only updates a mirrored world keep
  the plain overlap rule. The access setting and world settings are copied as for
  any mirrored world, unless the owner set them here and still holds the name. An
  operator block stands.
- **Local not outdated.** The world is refused as before, and only the scenes the
  mirror wrote there are retired. The listing's timestamps decide this before
  anything is downloaded. The write decides again on the hash-checked entities, so a
  publish that lands during the download is compared too.
- **Read-through fills** never replace a local publish.
- **Place removal.** The explore catalogue then prunes the world's local row. That is
  not a place removal while the world is mirrored and one of its scene places is
  active, so no removal is queued. World bans and scene-admin grants on a retained
  place identity carry over to the mirrored scene.

## Forwarded publishes

With `WORLDS_FORWARD_UPSTREAM_URL` set, this node forwards local publishes and
mirrors the upstream's scene list after acceptance. Set the forwarding target
and mirror source to the same base URL. The receipt compares the URLs after
trimming whitespace and trailing slashes.

- **Rule.** A local scene with an accepted outbox receipt for this mirror's
  source leaves the timestamp comparison once that receipt is in the pass's
  snapshot. The decision under the world lock checks the same receipt id to
  catch a re-publish during upstream reads. A blocked world or one with an
  active scene still queued or sending to the mirror source keeps its local
  scenes. Accepted forwards are excluded from both sides of the comparison.
  An older, disjoint local scene with no accepted receipt keeps the world
  local unless an independent upstream deployment outdates it. This can
  leave the local scene visible here while the accepted scene is already
  published upstream.
- **Handover.** When no other local scene stands, the scheduled pass mirrors
  the complete current `/scenes` listing. A listed local scene keeps its place
  identity and storage when converted in place. For an accepted scene missing
  from the listing, the pass verifies absence with an entity-filtered request
  before retirement. An accepted scene can retire after confirmed removal
  while a different local scene keeps the world local. An unlimited pass
  checks uncatalogued worlds with accepted local scenes or active mirrored
  scenes from that source, within any explicit name filter. A bounded pass
  checks its first-N catalogue names. A confirmed empty listing retires
  the matching scenes and preserves creator-owned settings and grants.
  A previously listed world missing from the index goes through the same
  per-world confirmation before its held scenes retire. A mirror-only
  world with no local state can be evicted after its last scene is
  confirmed absent.
- **Catalogue coverage.** An `/index` response at the upstream ceiling of
  10,000 rows keeps the previous catalogue entries and records the pass as
  incomplete with a page-limit warning. The mirror continues reconciling
  its scheduled names using their per-world listings. Below the ceiling,
  the removal fuse still rejects unusually large drops. The operator
  verifies complete catalogue coverage before enabling the scheduled mirror.
  An incomplete index can still prevent discovery of new worlds.
- **Hosting.** Mirrored scenes are served as copies. Handover ends local
  authoritative SDK7 hosting and local collaborator and `logsPermissions`
  eligibility for those scenes. Mirrored scenes stop counting toward local
  personal-world quotas.
- **Access and settings.** The mirror reads access before committing a
  takeover. A failure or an unknown access type leaves the local world in
  place. Access and settings set here stay in effect while the
  recorded owner still holds the name. Upstream deployment and streaming
  grants are not imported.
- **Receipt limits.** The operator can read forwarding status under
  `/admin/forwarding`. The sweep retains an accepted receipt while its scene
  is still active locally. Once the scene is handed over or retired, the
  receipt follows the regular 30-day retention window. A response lost after
  an upstream commit and a rejection provide no affirmative receipt, including
  deployments relying on a grant the upstream does not know. A receipt
  swept before this change also provides no evidence after upgrade. In
  those cases the ordinary timestamp and tie rules apply. A read-through fill leaves the
  local publish in place.

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
  on the upstream's word as for any mirrored world. That scene reaches the local
  scene's storage only when it covers the local scene's whole footprint.
- A deployment grant over part of a local scene's footprint cannot reach that
  scene's storage through a takeover: the upstream scene on that part starts
  with a new identity.
- A local identity that a takeover does not keep is retired as a replacing publish
  retires one: its storage stays, and its grants no longer resolve to a scene.
- Every pass reads the `/about` and `/scenes` of each listed world that this node
  keeps local. Entities and files are fetched only when the upstream is newer.
