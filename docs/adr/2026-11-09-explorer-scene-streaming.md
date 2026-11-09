# ADR: The Explorer streams scenes in waves and shows a stand-in until each is ready

- Status: Accepted
- Date: 2026-11-09

## Context

unity-explorer brings the scenes around the player in gradually. Scenes within a 20-parcel ring
are partitioned by distance and facing; a scene whose nearest parcel is under 32 m runs live and
keeps running until it is 48 m away; the rest show a LOD, a cheap stand-in made from the
scene's own models under 80 m (LOD 0, placed from a descriptor the asset pipeline publishes next
to the scene) and a single decimated mesh beyond (LOD 1). Scene loads are admitted a few at a
time, nearest first (`ScenesRequestBatchSize`), under loading-slot and memory budgets, and a
scene's stand-in stays on screen until the live scene is ready, when the live scene takes over
the models the stand-in already loaded.

The Bevy Explorer had one radius and one stand-in. `process_scene_lifecycle` spawned every scene
within 10 m in the frame it came into range, each with its own script thread, after holding
everything back until the player's own scene was up. A loading scene was parked under the world
with animated walls round its parcels, then raised in one frame at its fifth tick. Beyond the
live radius the only stand-in was a baked imposter tile, where one existed. Two defects made
the edge of the live radius churn: the keep band compared a parcel's distance against the extra
unload distance alone, so the "Scene Unload Distance" setting did nothing and a live scene
unloaded the moment the player stepped past the load distance; and nothing ordered the spawns
that the nearest-first work (`2026-11-02-explorer-nearest-first-priority.md`) left for later.

## Decision

Scene streaming is one ladder of visual states per scene, decided by distance with hysteresis,
and loads are admitted in waves.

- **Hysteresis at the live edge.** A scene spawns when a parcel is within the load distance and
  is kept until it is past load plus unload, the band the settings describe. A scene the player
  stands in, a portable and a system scene are never in the band.
- **Waves.** Beyond the player's own scene, portables and system scenes, which spawn at once, at
  most `SCENES_PER_WAVE` (6) scenes are loading at a time. A scene holds a wave slot from its
  spawn until it is ready, broken, frozen or failed, or for `WAVE_SLOT_TIMEOUT` (20 s), after
  which it no longer holds the scenes behind it back. The free slots go to the newly required
  scenes in partition order, the same key the download slots use: nearest parcel first, then
  in front of the camera before behind. The partition of a scene that is not yet spawned is
  measured from the parcels of its pointers in range.
- **Stand-ins, one shared readiness rule.** A loading scene shows nothing of its own: the
  stand-in for its parcels stays until the scene is ready (`load_timing::scene_is_ready`: five
  ticks, broken or frozen), and is removed in the frame the scene is raised. The ladder, from
  near to far: the live scene; a near LOD made of the scene's own glTF files placed from the
  asset pipeline's initial-scene-state descriptor, loaded under the scene's content paths with
  the scene's loader settings so that the live scene's containers find them in the asset cache
  and take them over; a far LOD, the pipeline's single mesh for the scene; the baked imposter
  tile. A scene without a descriptor or mesh falls to the next rung. Parcels with a ready
  stand-in are excluded from the imposters, so imposters draw only beyond the LOD range.
- **Download tier.** LOD files download in a tier of their own after the live scenes', so a
  stand-in never delays the scene it stands in for.

- **The near LOD's files are the scene's.** The descriptor
  (`abgen-cdn.<base>/LOD/lods-unity/manifests/{sceneId}_InitialSceneState.json`, the same host
  unity-explorer reads it from, resolved like every other service from the base domain or its
  override) names each model by content hash; the stand-in finds the file in the scene's
  content map, registers the map as the scene's collection and loads
  `$content_file/{scene}/{file}` with the scene's loader settings, so the live scene's
  containers get the same handles. Its meshes are uploaded, its root nodes turned, its collider
  nodes hidden and its materials bound to the scene's bounds the way the live scene's are, so
  nothing moves at the hand-over; lights and animations stay off. A scene wholly behind the
  camera starts no stand-in. The realm's active-entities response, which already carries every
  scene's content map, is kept for this instead of fetching each definition again.

The far LOD lands in a follow-up change on this decision.

## Rejected

- **Spawning everything in range in one frame, ordered.** Ordering alone fixes nothing: the cost
  is the script threads and definition loads starting together, which hitches the frame.
- **A wave as a fixed batch that waits for the whole batch.** Six slots that refill one at a time
  keep the pipeline full; a batch that waits for its slowest member idles the others.
- **Lifting the player's-scene-first deferral.** With waves it could go, but the player's scene
  is the one scene that must come up before anything else competes for the frame. It stays.
- **A catalyst copy of the LOD files.** The client reads them from the generator's CDN as
  unity-explorer does; a deployment that mirrors or converts them serves that host itself.
- **unity-explorer's 32/48 m live radius now.** The live radius and the unload margin are
  settings; raising the defaults is a measurement, after waves and the near LOD exist.
- **Time-based reveal of a ready scene.** The scene is raised in one frame as before; a
  cross-fade between a stand-in and the scene is its own change on the stand-ins.

## Consequences

- A live scene no longer flips at the load edge; the "Scene Unload Distance" setting works.
- Entering a dense area spawns six scenes, nearest and in front first, then one more as each
  comes up, instead of every scene in range in one frame.
- A loading scene within 80 m shows its own models as soon as they have all loaded, where the
  pipeline has a descriptor for it; elsewhere its imposter tile where one is baked, and nothing
  where none is. The walls are gone.
- The stand-in's models are the live scene's first downloads done early: when the scene comes
  up, its containers find them loaded.
- A scene stalled in loading holds a wave slot for 20 s, then the wave moves on with it still
  loading.
