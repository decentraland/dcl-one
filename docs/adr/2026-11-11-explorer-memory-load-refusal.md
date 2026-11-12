# ADR: The Explorer refuses new scenes and departed scenes' downloads while engine memory is full

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
  kept and keep loading; how many were held back is published as `RefusedScenes`.
- **Downloads.** While refusing, the request slots hold the requests of departed scenes (the
  `Departed` tier). A held request keeps its place and is granted once the refusal ends, or at
  once when its scene is registered again. Every other request keeps downloading: the player's
  scenes, avatars, the other live scenes, and requests made by URL, so a scene already spawned
  finishes loading rather than stalling half-built.
- **Visibility.** The Memory and limits debug section shows the latch, the number of scenes held
  back and the held tier; lowering the session memory limit there triggers it.

## Rejected

- **Holding every download below the player's scenes.** A neighbour already spawned would stall
  half-loaded until memory drops, which keeps its memory without giving anything for it.
- **Holding the `Other` tier.** A request made by URL carries no scene, so the slots cannot tell
  a live scene's NFT image, URL font or SDK6 adaption layer from an imposter archive; holding the
  tier would stall the player's own scene.
- **Unloading or shrinking the live radius at Warning.** Stepping scenes down needs something to
  step down to; it is a separate decision.
- **Per-scene memory estimates.** Fixed per-parcel figures are guesses; the measured reading is
  what is available.
- **Reusing `LiveScenes::block_new_scenes`.** It stops every spawn, the player's own scene and
  system scenes included.

## Consequences

- A session that reaches full memory stops growing from new scenes and background downloads, and
  resumes once memory falls below the warning threshold.
- On native the reading counts the engine process only, not the scene runtime process or other
  applications, so refusal starts late on a machine whose memory other programs already use.
- The refusal releases nothing by itself; freeing memory under pressure is separate work.
- Imposter archives and other requests made by URL still download while refusing. Holding them
  needs each request to name its owner, which URL requests do not yet do.
