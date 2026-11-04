# ADR: The Explorer reads profiles from the centralized catalyst, compact or full, and announces only deployed versions

- Status: Accepted
- Date: 2026-11-04

## Context

unity-explorer reads profiles through one repository in two tiers. The full tier is the whole
profile; the compact tier (`Profile.CompactInfo`: name, claimed-name flag, name colour, face
thumbnail) serves lists: friends, chat, communities, notifications, place creators, avatar
textures. Behind a feature flag (`USE_CENTRALIZED_PROFILES`) both tiers are batched POSTs to the
asset-bundle registry (`/profiles`, `/profiles/metadata`); without it they go to a catalyst's
lambdas, which have no compact route, so the tier is forced to full. A remote player's catalyst
comes from their LiveKit metadata. Publishing its own profile, unity-explorer snapshots the
current one, shows the edit in the world, deploys, reads the deployed version back, and either
announces the new version to other players or puts the snapshot back.

The Bevy Explorer had one tier. It batched full profiles to the registry of each owner's
environment, judged by the lambdas endpoint the owner's peer announced in its LiveKit metadata,
fell back to a single GET on that peer-named catalyst, then asked the peer. It announced a new
version of its own profile before deploying it and kept a rejected edit; only the HUD's Revert
button undid it, by deploying the old look again as a newer version.

In dcl-one every backend area is one catalyst. Its `asset-bundle-registry.<domain>` routes are a
read-through of Decentraland's registry (empty on an `island` node), so a profile deployed to the
node missed every batch, was fetched one address at a time and, never counting as final, was asked
of the registry again every 30 s. The parity card A6 (Profiles) asked for the compact tier and for
an engine-side revert of a failed publish.

## Decision

- **One source: the centralized catalyst.** The Explorer fetches profiles from
  `Service::Catalyst`'s lambdas (`peer.<base>`, or `--catalyst`): `POST /lambdas/profiles` for
  full profiles and `POST /lambdas/profiles/metadata` for compact ones, at most 100 ids a request.
  There is no flag between endpoints and no catalyst taken from another player: the LiveKit
  metadata path is gone. The catalyst's answer is final; a peer's own answer stays the source for
  guests, who have no deployed profile.
- **The catalyst serves the compact tier from its own deployments.**
  `POST /lambdas/profiles/metadata` answers `{"ids": [...]}` (at most 1000) with upstream's
  `ProfileMetadataDTO` (`decentraland/asset-bundle-registry@f9578d4a`, `getMetadata`): `pointer`,
  `hasClaimedName`, `name`, `nameColor` when set, and `thumbnailUrl`, the processed profile's face
  snapshot url. It projects the documents `/lambdas/profiles` returns (this node's deployments, the
  read-through, the ownership and claimed-name checks, the per-address cache), in request order, so
  the compact and full answers agree.
- **The tier is per address in the Explorer's profile cache.** Each cached address remembers the
  most any reader has asked of it: `ProfileManager::get_compact` (and the face and name readers
  built on it, and the HUD's `getCompactProfiles` for list rows) asks compact; `get_data`, an
  announced player and our own profile ask full. A compact answer never replaces full data, full
  data answers compact reads, and an address read compact and then full is fetched full at once.
- **A catalyst without the compact route serves compact reads from full profiles.** A compact POST
  answered 404 (Decentraland's catalysts) or 405 (a catalyst whose `/lambdas/profiles/{id}` takes
  the path) makes every fetch full for the rest of the session.
- **Other players only hear versions the server holds.** While an edit of our own profile deploys,
  the version announcement, its keepalive and the answer to a peer's `ProfileRequest` carry the
  profile from before the edit; the new version is announced once the deploy lands. A failed
  deploy puts that profile back in the world (the player's avatar, scenes' `PLAYER` data, the
  profile cache, the HUD's profile-changed stream) without deploying again.

## Rejected

- **Batching through `asset-bundle-registry.<domain>`.** On dcl-one it serves Decentraland's
  registry, not the node's deployments, and nothing on an `island` node.
- **Serving the node's own data from the registry routes instead of a lambdas route.** Those routes
  are unity-explorer's boot reads of Decentraland's registry; the node's profiles already live at
  `/lambdas/profiles`, and unity-explorer's catalyst url for the compact tier is
  `{lambdas}/profiles/metadata`.
- **A flag between a centralized service and a catalyst** (unity-explorer's
  `USE_CENTRALIZED_PROFILES`). The catalyst is the centralized service.
- **The catalyst a peer announces in its LiveKit metadata.** Another player could make every
  client call a server of its choosing, and the metadata will not carry it.
- **Fetch behaviour chosen per call** (unity-explorer's `FetchBehaviour` flags). The Explorer's
  cache keeps one fetch policy, so the tier rules live in one place.
- **Reverting from the HUD by deploying the old look again** (the previous behaviour). It deployed
  old content as a newer version, the engine kept the rejected edit and carried it into the next
  save, and other players had already been told the rejected version.

## Consequences

- Other players see a profile edit once it has deployed, as in unity-explorer, not the moment it is
  made; the player sees it at once.
- Profiles deployed to a dcl-one node resolve in batches and settle; nothing re-probes them.
- On a catalyst without the compact route (`decentraland.org`, the default base domain) the first
  compact read costs a 404, then compact reads cost full profiles.
- `asset-bundle-registry.<domain>/profiles*` keeps serving Decentraland's registry to
  unity-explorer.

## Follow-ups

- Drop the `--asset-bundle-registry` service, which no Explorer code reads any more (the engine's
  service table, the HUD's generated tables, the headless launcher), and the login's
  `--profiles` lambdas hint (the engine and the react-web, ui3 and e2e drivers), which always names
  the same catalyst.
- unity-explorer's four-second deploy window and its skip of an identical profile are not ported.
