# ADR: The Explorer keeps live scenes through an unload margin and spawns them in waves

- Status: Accepted
- Date: 2026-11-09

## Context

`process_scene_lifecycle` decides which scenes are live. A scene is live when the nearest point
of one of its parcels is within the load distance ("Scene Load Distance", 10 m by default). Two
problems made that set unstable and its growth uncontrolled:

- **No margin at the edge.** "Scene Unload Distance" is described as the additional distance,
  above the load distance, at which a live scene is despawned. The keep band compared a parcel's
  distance against that extra distance alone, so at the defaults (10 m and 10 m), and whenever the
  load distance was at least the extra distance, a live scene was despawned the moment the player
  stepped past the load distance. Crossing the edge back and forth despawned and respawned the
  scene each time: its script restarted, its state reset, its files were requested again.
- **Everything in range spawned in one frame.** Once the player's own scene was up, every newly
  required scene spawned in the same frame, in hash-map order, each starting its script runtime
  and definition load together. At 10 m that is a handful of scenes; at the setting's 150 m
  maximum it is more than a hundred, and the frame hitches.
- **The live edge too close to reach.** At 10 m and a run of 11 m/s, a player reaches a scene's
  edge less than a second after it starts loading, before it can show.

The download order is already nearest first (`2026-11-02-explorer-nearest-first-priority.md`);
the spawn order was left to this decision.

## Decision

- **Hysteresis at the live edge.** A scene spawns when a parcel is within the load distance and
  is kept until it is past load plus unload, the band the settings describe. The scene the player
  stands in, portables and system scenes are always required and never in the band.
- **A live ring under the distance.** Whatever the load distance, a scene with a parcel within
  `LIVE_RING` (1) of the player's parcel, the Chebyshev ring of the eight parcels around it, is
  required, and a live scene is kept while a parcel is within `LIVE_KEEP_RING` (2). At a run that
  leaves about 1.5 s before the player reaches the scene's edge. A larger load distance still
  widens the live area.
- **Waves.** Beyond the player's own scene, portables and system scenes, which spawn at once, at
  most `SCENES_PER_WAVE` (6) scenes are loading at a time. The limit is on scenes loading, not on
  scenes live: a scene holds a wave slot from its spawn until it is ready (past its sixth tick,
  broken, frozen or failed), or for `WAVE_SLOT_TIMEOUT` (20 s), after which it no longer holds the
  scenes behind it back. A slot is refilled as soon as it frees.
- **Order.** The free slots go to the newly required scenes by `LoadPriority`, the key the
  download slots use: the Chebyshev parcel ring first, then in front of the camera before behind,
  then distance. The partition of a scene that is not yet spawned is measured from the parcels of
  its pointers in range and in the live ring.

How many scenes are live stays set by the live ring and the two distance settings; this decision
adds no cap on the total and no memory reaction.

### Near LOD

A scene in range that is not on screen yet (not live, or live and still loading) gets a
stand-in made of its own glTF files (`scene_lod.rs`).

- **Range in parcel rings.** A stand-in starts when the scene's partition `parcel_distance`, the
  Chebyshev ring of its nearest parcel around the player's, is at most 3 (about 48 m), and is
  kept until that ring passes 4 (about 64 m). A scene wholly behind the camera starts none.
  Never in preview.
- **Descriptor.** Where the models go comes from the asset pipeline's initial-scene-state
  descriptor, `{abgen-cdn}/LOD/lods-unity/manifests/{sceneId}_InitialSceneState.json`, served
  by the asset-bundle generator CDN (`Service::AbgenCdn`, `abgen-cdn.<base>` or
  `--abgen-cdn`). It names each model by content hash and gives its transform. A missing
  descriptor means no stand-in for that scene.
- **Shared handles.** The stand-in finds each file in the scene's content map, which the
  realm's active-entities response already carries, and loads it under the scene's content
  path with the scene's glTF loader settings. A content-file path takes the content map's key
  form (relative, lowercase), so the live scene's `GltfContainer` asks for the same asset path
  whatever case its `src` uses, gets the same `Handle<Gltf>`, and nothing downloads or parses
  twice.
- **Download tier.** While no live scene holds its hash, the stand-in's downloads rank in
  `Tier::Lod`: after every live scene's and before requests that are no scene's. Once the
  scene spawns, its own registration takes the hash to the scene tier.
- **Hand-off.** Nothing shows until every model has loaded or failed; then the stand-in shows
  in one frame. It goes in the frame the live scene's content is on screen
  (`RendererSceneContext::content_shown`). Imposters skip the parcels of a shown stand-in.
- **Collisions.** A shown stand-in's `*_collider` nodes (or nodes under one) are physics
  colliders on its root's `SceneColliderData`, with the mask the live scene's container gives
  them by default (`CL_PHYSICS`; skinned ones none). Player movement casts against every
  `SceneColliderData`, so a player who reaches the scene before it is live walks on what they
  see; pointer events and triggers query live scenes only. The colliders go with the stand-in.
- **Walls.** While a scene's stand-in is shown its loading walls are hidden; a scene without a
  shown stand-in keeps them.
- **Debug view.** The Scene stand-ins debug section counts near and far LODs by state and can
  tint the near LOD's models green and the far LOD's mesh blue (`/lod_tint`). The tint changes
  only the stand-in's own materials, never the live scene's.

### Far LOD

Past the near LOD's keep ring a scene that is not on screen gets a far LOD instead: the asset
pipeline's single mesh for the scene (`scene_lod.rs`, `LodLevel::Far`).

- **Mesh.** `{abgen-cdn}/LOD/lods-unity/lods/{sceneId}_1.glb` (the scene id lowercase), one glTF
  placed at the scene's base parcel and dressed like the near LOD's models. None published, or
  one the loader refuses, leaves the scene's parcels to the imposters.
- **Range in parcel rings.** It starts where the near LOD's keep band ends: its scene's nearest
  parcel past ring 4 (about 64 m) and within ring 20 (about 320 m), the scene not wholly behind
  the camera. Inside ring 4 an existing far LOD is kept until the near LOD shows, so the scene is
  never bare between the two; one is not started there. Beyond ring 20 only the imposters remain.
- **Edge on imposter tiles.** Imposters are tiles of 2^level parcels a side, and any tile with a
  stand-in's parcel in it is split down to single parcels. A far LOD is therefore shown only if
  every parcel of its scene lies where the imposters would draw a tile of level 1 or less
  (`FAR_LOD_MAX_TILE_LEVEL`, 2 parcels a side), as the imposters lay out their tiles around the
  focus (`ImposterTiling`, set by the imposters). No larger tile ever holds a far LOD, so the
  edge between far LODs and imposters falls on a tile boundary and the splitting stays among the
  small tiles. At the default imposter distances that edge lies between about 200 m and 290 m;
  ring 20 bounds it where the tiles reach further (a camera far from the player).
- **Download tier.** The mesh is a url download. Its path names the scene it downloads for
  (`IpfsKey::Owner`, the `&owner` key; `IpfsPath::scene_hash` returns it), so it ranks in
  `Tier::Lod` with the near LOD's files rather than with the requests that are no scene's.
- **Quantized meshes.** The meshes require `KHR_mesh_quantization` (16-bit positions, 8-bit
  normals, 16-bit texture coordinates). The vendored glTF loader accepts files that require it
  and reads the integer attributes as the floats they stand for
  (`rendering/bevy-explorer/docs/guidelines/bevy-fork.md`).
- **Hand-off.** Like the near LOD it shows in one frame once loaded, and goes in the frame the
  live scene's content is on screen. Imposters skip its parcels and its scene's loading walls
  are hidden while it is shown.

The removal of the loading walls is a later decision.

## Rejected

- **Spawning everything in range in one frame, ordered.** Ordering alone fixes nothing: the cost
  is the script runtimes and definition loads starting together.
- **A wave as a fixed batch that waits for its slowest member.** Slots that refill one at a time
  keep loading continuous; a batch idles the others while one scene is slow.
- **Lifting the player's-scene-first deferral.** The player's scene is the one that must come up
  before anything else competes for the frame. It stays.
- **Raising the default load distance instead of a ring.** The distance is measured to the
  nearest point of a parcel, so no metre value covers the ring around the player's parcel from
  every point of it; the ring does, and leaves the setting free to widen the area.
- **A live ring of 2.** Up to 25 scripts running at once among one-parcel scenes, against 9 at
  ring 1, before the time a scene takes from spawn to shown has been measured.
- **A separate simplified near model.** It would need colliders of its own, and a scene the
  player enters would download a second copy of its models. The scene's own files carry their
  collider nodes already.
- **Far LODs out to ring 20 regardless of the tiles.** Every 4×4 tile between 200 m and 320 m
  touching a far LOD would be split into single-parcel imposters, each its own load, and a tile
  swap shows the old tile over the LOD until its children load.
- **Far LODs cut off at the tiles' parcels.** Scene materials can clip to parcels, but a scene
  on the edge would show part mesh and part imposter.
- **A near-LOD range in metres.** Rings are what the partition, the download order and the
  waves already measure, and a keep ring one wider than the start ring stops a player on the
  edge from starting and dropping a stand-in every few steps.

## Consequences

- A live scene no longer respawns at the load edge, and "Scene Unload Distance" does what its
  description says.
- Entering a dense area with a large load distance spawns six scenes, nearest ring and in front
  first, then one more as each comes up, instead of every scene in range in one frame.
- At the default 10 m the live ring decides: up to 9 scenes among one-parcel scenes, 25 inside
  the keep ring. Waves rarely queue a scene there; they matter once the load distance is raised.
- Whether 1.5 s is enough depends on the time a scene takes from spawn to shown, which has not
  been measured.
- Out to the edge of the small imposter tiles (about 200 m to 290 m at the default imposter
  distances), a scene the pipeline has meshed shows that mesh where it would have shown its
  imposter tile.
- The far LOD's textures are not compressed to BC7: the texture processor compresses only
  content-addressed files, and url downloads live in the web cache, so a mesh decodes its
  textures on every load. Compressing them is a change to the processor's cache targets.
- A scene stalled in loading holds a wave slot for 20 s, then the wave moves on with it still
  loading.
- Loading scenes keep their animated walls; what shows while a scene loads is a later decision.
- Within three rings, a loading scene shows its own models as soon as they arrive instead of
  its walls, and the live scene reuses those models when it comes up.
- A scene the asset pipeline has not processed has no descriptor and keeps its walls.
- A stand-in's collisions are the defaults only: a container's collision masks, `MeshCollider`
  components set by scene code, primitives a script creates and things a script moves are
  absent until the scene is live.
