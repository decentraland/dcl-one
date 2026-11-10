# ADR: The Explorer reads profiles from the centralized catalyst, compact or full, and announces only deployed versions

- Status: Accepted
- Date: 2026-11-04

## Context

Avatar rendering needs full profiles, while friends, chat, communities, notifications,
creator lists and avatar face textures need only names, claimed-name flags, name colors and
face thumbnails. One cache
must satisfy both sets of readers without letting fetch policy vary by caller.

Every backend area in dcl-one is served by one catalyst. The asset-bundle registry routes are
a read-through of Decentraland's registry and are empty on an `island` node; they cannot be the
authority for profiles deployed to that node. Profile reads must use the catalyst's own
deployments and avoid endpoints supplied by another player.

A local edit should appear immediately while it deploys, but peers must hear only a version
the server holds. A failed publish must restore the confirmed profile without deploying the
old content again or reusing a version that another peer may already have observed.

## Decision

- **One source: the centralized catalyst.** The Explorer fetches profiles from
  `Service::Catalyst`'s lambdas (`peer.<base>`, or `--catalyst`): `POST /lambdas/profiles` for
  full profiles and `POST /lambdas/profiles/metadata` for compact ones, at most 100 ids a request.
  The one exception is the compact tier on Decentraland's own deployments, below. There is no
  flag between endpoints and no catalyst taken from another player: the LiveKit metadata path is
  gone. The catalyst's answer is final; a peer's own answer stays the source for guests, who have
  no deployed profile.
- **The catalyst serves the compact tier from its own deployments.**
  `POST /lambdas/profiles/metadata` answers `{"ids": [...]}` (at most 1000) with upstream's
  `ProfileMetadataDTO` (`decentraland/asset-bundle-registry@f9578d4a`, `getMetadata`): `pointer`,
  `hasClaimedName`, `name`, `nameColor` when set, and `thumbnailUrl`, the processed profile's face
  snapshot url. It projects the documents `/lambdas/profiles` returns (this node's deployments, the
  read-through, the ownership and claimed-name checks, the per-address cache), in request order, so
  the compact and full answers agree.
- **For now, Decentraland's own deployments read compact profiles from their asset-bundle
  registry.** Their catalysts have no compact route (`peer.decentraland.org` answered
  `POST /lambdas/profiles/metadata` with 405 on 2026-11-09), while
  `asset-bundle-registry.decentraland.org` and `.zone` answer `POST /profiles/metadata` with the
  same `ProfileMetadataDTO` (checked the same day). When the resolved catalyst's host is
  `decentraland.org` or `decentraland.zone`, or a host under one
  (`common::base_domain::decentraland_domain`), compact reads go to
  `https://asset-bundle-registry.<that domain>/profiles/metadata` (`compact_source` in the comms
  profile module). Full profiles, the login's read and the default looks stay on the catalyst.
  The switch follows the resolved catalyst, not the base domain: a dcl-one page on its own host
  and a native `--gateway` launch keep the default `decentraland.org` while their catalyst is a
  dcl-one node. This path is temporary and is to be removed once Decentraland's catalysts serve the
  compact route.
- **The tier is per address in the Explorer's profile cache.** Each cached address remembers the
  most any reader has asked of it: `ProfileManager::get_compact` (and the face and name readers
  built on it, and the HUD's `getCompactProfiles` for list rows) asks compact; `get_data`, an
  announced player and our own profile ask full. A compact answer never replaces full data, full
  data answers compact reads, and an address read compact and then full is fetched full as soon as
  any compact fetch already in flight for it lands.
- **A catalyst without the compact route serves compact reads from full profiles.** A compact POST
  answered 404 or 405 (Decentraland's catalysts answer 405: their `/lambdas/profiles/{id}` takes
  the path) makes every fetch full for the rest of the session; a compact POST that fails any other
  way has its addresses read whole.
- **Other players only hear versions the server holds.** While an edit of our own profile deploys,
  the version announcement, its keepalive, the Pulse handshake, the ws-room's first announcement
  and the answer to a peer's `ProfileRequest` carry the profile from before the edit; the new
  version is announced once the deploy lands. A failed deploy puts that profile back in the world
  (the player's avatar, scenes' `PLAYER` data, the profile cache, the HUD's profile-changed stream)
  without deploying again.
- **A newer edit does not cancel an older one still deploying.** Both deploys run to the end. If
  the older one lands while the newer one still deploys, it is what peers hear and what a failure
  of the newer one puts back; if it lands after the newer one failed, the engine takes it, since
  the server holds it.
- **An edit's version is new for the session.** It goes above every version this session gave the
  account, not just above the profile held: a deploy that failed here may still have reached the
  server (an error after the catalyst took it), and a peer that read that version skips any other
  profile announced under it.

## Rejected

- **Batching through `asset-bundle-registry.<domain>`.** On dcl-one it serves Decentraland's
  registry, not the node's deployments, and nothing on an `island` node. In that fetch cascade,
  a node-deployed profile misses the batch, falls back to a single-address GET, and remains
  non-final, so it is asked of the registry again every 30 s.
- **Serving the node's own data from the registry routes instead of a lambdas route.**
  Registry routes serve Unity Explorer's boot reads of Decentraland's registry; the node's
  deployed profiles already live at `/lambdas/profiles`. The compact projection belongs beside them at
  `/lambdas/profiles/metadata`. Unity Explorer's `USE_CENTRALIZED_PROFILES` flag selects registry
  `POST /profiles` and `/profiles/metadata`; otherwise it reads catalyst lambdas. Its
  `RealmProfileRepository.PostUrl` constructs that compact-tier path for a catalyst, but it
  currently forces full-profile reads there.
- **A flag between a centralized service and a catalyst.** The catalyst is the centralized
  service; the registry stands in only for the compact route Decentraland's catalysts lack,
  chosen by where the catalyst is.
- **The base domain as the registry switch.** A dcl-one page served from its own host, and a
  native `--gateway` launch, keep `decentraland.org` while their catalyst is a dcl-one node, whose
  deployments Decentraland's registry does not hold.
- **Full profiles from the registry too** (Unity Explorer with the flag on). The catalyst already
  batches full profiles and is authoritative for the login's read and the read-back of our own
  deploy, which a registry lagging behind it could answer with an older version.
- **The catalyst a peer announces in its LiveKit metadata.** Another player could make every
  client call a server of its choosing, and the metadata will not carry it.
- **Fetch behaviour chosen per call.** The Explorer's cache keeps one fetch policy, so the tier
  rules live in one place.
- **Reverting from the HUD by deploying the old look again** (the previous behaviour). It deployed
  old content as a newer version, the engine kept the rejected edit and carried it into the next
  save, and other players had already been told the rejected version.

## Consequences

- Other players see a profile edit once it has deployed; the player sees it at once.
- Profiles deployed to a dcl-one node resolve in batches and settle; nothing re-probes them.
- On Decentraland's deployments compact reads cost compact answers from their registry. Another
  catalyst without the route costs one 404 or 405, then full profiles.
- A Decentraland profile's compact and full tiers come from different services. Right after a
  deploy the registry can lag the catalyst; a compact answer never replaces full data.
- `asset-bundle-registry.<domain>/profiles*` keeps serving Decentraland's registry to
  Unity Explorer.
- A four-second deploy-coalescing window and identical-profile suppression remain unimplemented.

## Follow-ups

- Remove the registry path once Decentraland's catalysts answer `POST /lambdas/profiles/metadata`:
  `compact_source`'s registry branch in the comms profile module and
  `common::base_domain::decentraland_domain`. Compact reads then go to the catalyst everywhere.
