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
- **Waves.** Beyond the player's own scene, portables, system scenes and the scene an imposter is
  being baked from, which spawn at once, at most `SCENES_PER_WAVE` (6) scenes are loading at a
  time. The limit is on scenes loading, not on
  scenes live: a scene spawned into a wave slot holds it until it is ready (past its sixth tick,
  broken, frozen or failed), or for `WAVE_SLOT_TIMEOUT` (at least 20 s: app time stands still
  while a web tab is in the background and caps each frame's step), after which it no longer
  holds the scenes behind it back. The slot belongs to the scene from its spawn, whatever role it
  takes later; a scene that spawned at once holds none. A slot is refilled as soon as it frees and
  is never taken back from a scene that holds it.
- **Order.** When more scenes are due than slots are free, the free slots go to them by
  `LoadPriority`, the key the
  download slots use: the Chebyshev parcel ring first, then in front of the camera before behind,
  then distance. The partition of a scene that is not yet spawned is measured from the parcels of
  its pointers in range and in the live ring.

How many scenes are live stays set by the live ring and the two distance settings; this decision
adds no cap on the total and no memory reaction.

### Near LOD

A scene in range that is not on screen yet (not live, or live and still loading) gets a
stand-in made of its own glTF files (`scene_lod.rs`). The stand-in has two parts, because its two
jobs cost different things: stopping a player from walking into a loading scene needs only
collider shapes in main-world memory, while showing the scene needs its textures and meshes on
the GPU.

- **Range in parcel rings.** A stand-in starts when the scene's partition `parcel_distance`, the
  Chebyshev ring of its nearest parcel around the player's, is at most 3 (about 48 m), and is
  kept until that ring passes 4 (about 64 m). Never in preview.
- **Colliders: by distance, never interrupted.** Every scene within the start ring starts its
  colliders part, whatever the facing, so a player backing into a loading scene is stopped like
  one walking in; the ring bounds how many. Once started, it goes only when the scene leaves the
  keep ring or its live content shows.
- **Visuals: by view, one cap.** At most "Max Near LODs" stand-ins draw at once (a Performance
  setting, 0 to 40, default 12), in view and behind the camera together. Scenes in view whose
  models are known start nearest first, by the download key (ring, then distance), while a place
  is free. With none free, a drawn stand-in behind the camera makes way for one in view, the
  farthest first; one in view never does, and turning the camera alone stops nothing. Lowering
  the setting stops the farthest over it, those behind the camera first. The cost of a stand-in
  is not known before its files arrive, and a guess from the download misleads (a scene of flat
  PNGs takes ten times its download in memory), so the cap counts scenes.
- **Definition.** A stand-in starts by loading the scene's entity definition as the live scene
  does, by hash or by the realm's urn, so the live scene gets the same handle and the entity
  file is fetched once. Nothing about a scene is kept before its stand-in starts: until then
  the realm's pointers give the scene's parcels within the keep ring, which is all its
  partition needs. The definition gives the base parcel, the parcels and the content map; the
  stand-in registers the content collection unless the live scene already has.
- **Descriptor.** Where the models go comes from the asset pipeline's initial-scene-state
  descriptor, `{base}/LOD/lods-unity/manifests/{sceneId}_InitialSceneState.json`, under the
  Bevy variants base (`IpfsIo::bevy_lod_descriptor`; by default the asset-bundle generator CDN,
  `abgen-cdn.<base>`). With variants off there is no descriptor and so no near LOD. It is an
  asset load under the scene's content path, so it goes through the
  ipfs cache and ranks with the stand-in's other downloads. It names each model by content hash
  and gives its transform. A descriptor that fails to load, most often because the pipeline has
  none for the scene, means no stand-in while the scene stays within the keep ring.
- **What the descriptor holds.** It lists only glTFs the scene deployed. An entry whose hash is
  not among the scene's deployed files, such as a model the scene loads from a url, is skipped
  and logged at debug level: the stand-in never fetches a url the scene chose before the scene
  runs. Primitive meshes, text and materials the scene's code sets are not in the descriptor at
  all. A scene whose models are all skipped keeps its walls.
- **The live container path.** Each model becomes a glTF container per part, named by its file
  in the scene's content map, which the live glTF systems (`gltf_container`) load and process a
  slice a frame under the instantiation deadline. A visuals container (`GltfPart::Visuals`) hangs
  from the stand-in's root and is dressed as a live scene's container is (skins, meshes,
  materials, lights, animations, collider nodes hidden) but builds no collider. A content-file
  path takes the content map's key form (relative, lowercase), so the live scene's
  `GltfContainer` asks for the same asset path whatever case its `src` uses and gets the same
  `Handle<Gltf>`. Each visuals container keeps its handle until its visuals stop or the stand-in
  goes, and the stand-in goes once the live scene's content is shown, so a container the live
  scene creates by then holds the same handle and its file is not parsed again. A file is parsed
  again when the live scene names it later, when the scene is frozen or broken (its content
  counts as shown at once), or when the scene deploys the same file under two names and uses the
  one the stand-in did not pick. The stand-in's root carries what those systems read from a scene
  root (the definition handle, a resource lookup, a component tracker and its parcels' bounds,
  `StandInBounds`) and no `RendererSceneContext`.
- **The colliders part.** Each model loads a second time for its colliders
  (`GltfPart::Colliders`), under a colliders root that is a hidden child of the stand-in's root
  with collider data and a resource lookup of its own, so meshes held in main-world memory only
  never reach a visuals container. Its asset path carries an `&part/colliders` key that the
  download ignores: the ipfs layer fetches and caches by content hash, so the file downloads
  once for both parts and the live scene. A separate path keeps these loader settings away from
  the shared asset (the first load's settings win): no textures, materials, lights or cameras,
  meshes in main-world memory only. Empty material usages register embedded images as 1x1
  placeholders, and every image the file references by uri resolves to one placeholder the
  reader answers without a request. The live node rules decide the colliders (collider and
  trigger names, masks from extras, the container's default masks, `mesh_to_parry_shape`);
  nothing is drawn. Once processed the container lets the glTF go, and only the collider shapes
  and collider meshes stay.
- **Converted files.** Starting a stand-in requests the scene's Bevy variants manifest, as a
  spawning scene does, so the stand-in loads the asset pipeline's converted files (BC textures,
  `g` models) and the live scene finds those already loaded.
- **Download tier.** While no live scene holds its hash, the stand-in's downloads rank in
  `Tier::Lod`: after every live scene's and before requests that are no scene's. Once the
  scene spawns, its own registration takes the hash to the scene tier.
- **Hand-off.** The root stays hidden until each of its visuals containers is processed; then
  the visuals show in one frame. If no model loaded, nothing shows and the scene keeps its
  loading walls; the colliders stay. The stand-in goes, both parts with it, in the frame the live
  scene's content is first on screen (`RendererSceneContext::content_shown`), and a scene whose
  content has been on screen never gets a stand-in again, even while its content is briefly
  blocked (an imposter bake), so a stand-in does not come back over live content. The live scene
  is raised from below the world at its fifth tick, before its content counts as shown, so the
  two overlap from then until the stand-in goes. Imposters skip the parcels of a stand-in whose
  visuals show.
- **Collisions.** The colliders part's collider nodes become colliders on its root's
  `SceneColliderData` through the live collider systems as soon as each model is processed,
  whether or not anything is drawn: player movement, collision resolution and foot IK cast
  against every `SceneColliderData`, so a player who reaches a loading scene walks on its
  colliders. Pointer events and triggers query live scenes only. The colliders go with the
  stand-in.
- **Walls.** While a stand-in's visuals show, its scene's loading walls are hidden; a scene
  without shown visuals keeps them.
- **Debug view.** The Scene stand-ins debug section counts colliders parts and visuals by state
  and can tint the
  near LOD's models green (`/lod_tint`). The tint changes only the stand-in's own materials, never
  the live scene's.

A far LOD and the removal of the loading walls are later decisions.

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
- **Colliders from the drawn stand-in.** Its colliders would then follow the visuals' cap and
  facing: a scene behind the camera, never drawn or dropped to make way for one in view, would
  let the player walk into it. Keeping drawn stand-ins behind the camera alive for their
  colliders needed a second, hidden total beyond the setting.
- **A near-LOD range in metres.** Rings are what the partition, the download order and the
  waves already measure, and a keep ring one wider than the start ring stops a player on the
  edge from starting and dropping a stand-in every few steps.

## Consequences

- A live scene no longer respawns at the load edge, and "Scene Unload Distance" does what its
  description says.
- Entering a dense area with a large load distance spawns six scenes, nearest ring and in front
  first, then one more as each comes up, instead of every scene in range in one frame.
- At the default 10 m the live ring decides: up to 9 scenes among one-parcel scenes, 25 inside
  the keep ring. Waves still queue on arrival there: once the player's own scene is up, the
  ring-1 neighbours become due in the same frame, up to 8 among one-parcel scenes against 6
  slots, and the 2 behind the camera wait until one of the first six is ready, so they don't get
  the ring's full lead. Waves matter most once the load distance is raised. Only a wave larger
  than the free slots is ordered; the newly required scenes are still filtered each frame.
- No preemption: a scene that has just come within the live ring waits behind farther scenes
  already loading, until one is ready or for about 20 s. Letting the live ring bypass the limit
  would put the eight ring-1 neighbours back into one frame on arrival.
- Whether 1.5 s is enough depends on the time a scene takes from spawn to shown, which has not
  been measured.
- A scene stalled in loading holds a wave slot for at least 20 s, then the wave moves on with it still
  loading.
- Loading scenes keep their animated walls; what shows while a scene loads is a later decision.
- Within three rings, a loading scene shows its own models as soon as they arrive instead of
  its walls, and the live scene reuses those models when it comes up.
- A scene the asset pipeline has not processed has no descriptor and keeps its walls.
- A stand-in's collisions are the defaults only: a container's collision masks, `MeshCollider`
  components set by scene code, primitives a script creates and things a script moves are
  absent until the scene is live.
- A stand-in shows only content-file glTFs. Url models, primitives, text and code-set materials
  appear when the scene is live; the primitives, text and materials need the descriptor to
  describe them.
