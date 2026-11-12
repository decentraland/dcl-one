# ADR: The Explorer admits only the player's loads while engine memory is full

- Status: Accepted
- Date: 2026-11-11

## Context

`MemoryStatus` samples engine memory and classifies it as Abundance, Normal, Warning or Full
(below 65 %, 65–70 %, 70–75 %, above 75 % of the cap). On native the reading is the engine
process's resident memory against physical RAM (or the cgroup limit); on the web it is the live
allocations of the engine heap against its 4 GiB ceiling. Until now nothing acted on Full: the
scene lifecycle spawned every scene that came into range, and the request slots granted every
download, however close the engine was to running out. On the web that ends in a crash.

## Decision

- **One latch.** `LoadRefusal` turns on when a reading is Full and stays on through Warning; it
  turns off once the reading is Normal or Abundance. With no reading nothing is refused. The
  margin between 75 % and 70 % keeps a reading that hovers at the threshold from admitting and
  refusing on alternate samples.
- **Scenes.** While refusing, `process_scene_lifecycle` spawns no new world scene. The scene the
  player stands in, portables and system scenes are still admitted. Scenes already spawned are
  kept; how many were held back is published as `RefusedScenes`. An imposter bake's scene is
  refused too, so a bake waits for the refusal to end.
- **Downloads.** While refusing, the request slots grant only the `Active` tier: the scenes the
  player stands in, portables and global scenes, the player's own body, wearables and emotes
  (equipped or last played), the avatar previews the player opens (the backpack, a passport,
  with the emotes they play), and files every scene runtime needs (the SDK6 adaption layer,
  queued under a fixed owner registered at that tier). A preview the player opens is never held,
  so trying on an outfit cannot leave the backpack waiting on memory the refusal does not free. Every other request is held: other avatars, the other scenes (spawned ones
  included), emotes of other avatars, departed scenes and requests made by URL. A held request
  keeps its place and is granted once the refusal ends, or at once when it rises into the
  `Active` tier: its hash is registered for the player, the player's scene shares its download,
  or the player walks into its scene.
- **Visibility.** The Memory and limits debug section shows the latch, the number of scenes held
  back and the first held tier; lowering the session memory limit there triggers it.

## Rejected

- **Holding departed scenes' downloads only.** Every other scene and avatar keeps growing the
  engine while it is already full, so the refusal barely slows growth.
- **Holding every download, the player's included.** The player's own scene could not start or
  finish, and its own avatar would stay incomplete, until memory drops; nothing here releases
  memory, so that can last the rest of the session.
- **Giving each URL request its scene as owner.** The owner would become part of the asset path,
  so a file several scenes load by URL would be loaded once per scene. Only the SDK6 adaption
  layer, which the player's scene needs to start, gets a fixed owner that keeps one path.
- **Unloading or shrinking the live radius at Warning.** Stepping scenes down needs something to
  step down to; it is a separate decision.
- **Per-scene memory estimates.** Fixed per-parcel figures are guesses; the measured reading is
  what is available.
- **Reusing `LiveScenes::block_new_scenes`.** It stops every spawn, the player's own scene and
  system scenes included.

## Consequences

- Scenes already spawned around the player stall half-loaded while the refusal lasts, and other
  avatars keep whatever they had loaded; both resume once memory falls below the warning
  threshold.
- The player's own scene can still load a URL file it needs (an NFT image, a URL font) only once
  the refusal ends.
- The refusal slows growth; it does not stop it. Still unaffected: the player's own loads, reads
  that hit a cache (the disk cache, the web service-worker cache, locally stored imposters),
  which never ask for a request slot; instantiating avatars, models and textures from data
  already downloaded; and entity resolution (`active_entities`), which posts outside the request
  slots, keeps collectible and scene pointers resolving and stores collectible results that are
  never evicted.
- When the refusal ends, every scene it held back spawns in the same frame, and the latch has
  only the 5 % between 70 % and 75 % to absorb that backlog.
- On native the reading counts the engine process only, not the scene runtime process or other
  applications, so refusal starts late on a machine whose memory other programs already use. The
  allocator may also keep freed pages resident, so the reading can stay between 70 % and 75 %
  after usage has dropped and keep the refusal on.
- While a held request waits, the download priorities are published every frame, so that a
  neighbour the player walks into is released at once.
- The refusal releases nothing by itself; freeing memory under pressure is separate work.
