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
  The one exception is the compact tier on Decentraland's own deployments, below. There is no
  flag between endpoints and no catalyst taken from another player: the LiveKit metadata path is
  gone. The catalyst's answer is final; a peer's own answer stays the source for guests, who have
  no deployed profile, and is taken only when every address in it is the sending player's.
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
  registry, not the node's deployments, and nothing on an `island` node.
- **Serving the node's own data from the registry routes instead of a lambdas route.** Those routes
  are unity-explorer's boot reads of Decentraland's registry; the node's profiles already live at
  `/lambdas/profiles`, and `{lambdas}/profiles/metadata` is the path unity-explorer's repository
  builds for a catalyst's compact tier (`RealmProfileRepository.PostUrl`), though it forces the
  full tier on catalysts today.
- **A flag between a centralized service and a catalyst** (unity-explorer's
  `USE_CENTRALIZED_PROFILES`). The catalyst is the centralized service; the registry stands in
  only for the compact route Decentraland's catalysts lack, chosen by where the catalyst is.
- **The base domain as the registry switch.** A dcl-one page served from its own host, and a
  native `--gateway` launch, keep `decentraland.org` while their catalyst is a dcl-one node, whose
  deployments Decentraland's registry does not hold.
- **Full profiles from the registry too** (unity-explorer with the flag on). The catalyst already
  batches full profiles and is authoritative for the login's read and the read-back of our own
  deploy, which a registry lagging behind it could answer with an older version.
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
- On Decentraland's deployments compact reads cost compact answers from their registry. Another
  catalyst without the route costs one 404 or 405, then full profiles.
- A Decentraland profile's compact and full tiers come from different services. Right after a
  deploy the registry can lag the catalyst; a compact answer never replaces full data.
- `asset-bundle-registry.<domain>/profiles*` keeps serving Decentraland's registry to
  unity-explorer.

## Follow-ups

- Remove the registry path once Decentraland's catalysts answer `POST /lambdas/profiles/metadata`:
  `compact_source`'s registry branch in the comms profile module and
  `common::base_domain::decentraland_domain`. Compact reads then go to the catalyst everywhere.
- unity-explorer's four-second deploy window and its skip of an identical profile are not ported.
