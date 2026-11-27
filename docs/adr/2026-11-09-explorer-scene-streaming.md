# ADR: The Explorer keeps live scenes through an unload margin and spawns them in waves

- Status: Accepted
- Date: 2026-11-09

## Context

`process_scene_lifecycle` decides which scenes are live. A scene is live when the nearest point
of one of its parcels is within the load distance (10 m by default). Two
problems made that set unstable and its growth uncontrolled:

- **No margin at the edge.** The unload distance is described as the additional distance,
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
  the live ring ("Live Scene Ring", default 1) of the player's parcel, at 1 the Chebyshev ring of
  the eight parcels around it, is required, and a live scene is kept while a parcel is within one
  ring further (2 by default). At ring 1 and a run that leaves about 1.5 s before the player
  reaches the scene's edge. The ring is the player's control ("Live Scene Ring"): no setting
  changes the load distance, which preview mode, the server and `/scene_distance` set, and a
  larger one still widens the live area.
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
  Chebyshev ring of its nearest parcel around the player's, is at most the near ring ("Near LOD
  Ring", default 3, about 48 m), and is kept until its scene is past the next ring (4, about
  64 m). Never in preview.
- **Colliders: by distance, never interrupted.** Every scene within the start ring starts its
  colliders part, whatever the facing, so a player backing into a loading scene is stopped like
  one walking in; the ring bounds how many. Once started, it goes only when the scene leaves the
  keep ring or its live content shows.
- **Visuals: by view, one cap.** At most the near LOD count of stand-ins draw at once (the near
  ring gives it, 12 at the default ring; see Settings), in view and behind the camera together. Scenes in view whose
  models are known start nearest first, by the download key (ring, then distance), while a place
  is free. With none free, a drawn stand-in behind the camera makes way for one in view, the
  farthest first; one in view never does. So with every place taken, turning the camera stops
  drawn stand-ins left behind as new ones come into view; they reload from the cache when the
  player turns back. Lowering the setting stops the farthest over it, those behind the camera
  first. The cost of a stand-in
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
  all. A scene whose models are all skipped gets no near LOD.
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
  the visuals show in one frame. If no model loaded, nothing shows and its parcels stay the
  imposters'; the colliders stay. The stand-in goes, both parts with it, in the frame the live
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
- **Debug view.** The Scene stand-ins debug section counts near LODs' colliders parts and the
  near and far LODs' visuals by state, and can tint the near LOD's models green and the far
  LOD's mesh blue (`/lod_tint`). The tint changes only the stand-in's own materials, never the
  live scene's.

### Far LOD

Past the near LOD's keep ring a scene that is not on screen gets a far LOD instead: the asset
pipeline's single mesh for the scene (`scene_lod.rs`, `LodLevel::Far`), a second stand-in of
the same kind.

- **Mesh.** `{base}/LOD/lods-bevy/lods/{sceneId}_1.glb` (the scene id lowercase): the asset
  pipeline's Bevy copy of the scene mesh. The Bevy variants client names it
  (`IpfsIo::bevy_far_lod`): `base` is the variants base, and it is offered while a base is set,
  the scene not in preview, not local, and not served from a loopback or file content server,
  and only on a GPU with BC textures, since its atlases are BC1 or BC7 KTX2 with their mip chain.
  It does not follow the variants client's session switch (the manifests' and texture variants'
  breaker, which stays off once tripped): an outage of the LOD service is the far LOD breaker's
  (below), which resumes far LODs once the service answers again. Until the GPU reports its BC support the far LOD waits. The glTF loader hands
  embedded KTX2 to the host, which unwraps BC to DDS, so they stay compressed in memory. Like a
  `glb` variant it is asked for at its own path: its Brotli copy exists only when it is smaller,
  and the server sends it to a browser that accepts `br`. None offered or published, or one the
  loader refuses, leaves the scene's parcels to the imposters. The content server judged is the
  realm's (`/about`, or the server a loaded urn registered for the scene); a far LOD loads no
  urn, so on a realm whose scenes come from another server, such as a local city loader, that
  server is not judged, and each scene in the band costs one request a session that can only
  miss.
- **From the scene's hash alone.** The far LOD loads no entity definition. The url comes from
  the scene's hash, which the realm's pointers give, and the root goes at the scene's base
  parcel (the mesh is relative to it) as the realm listed it: the active-entities response that
  fills the pointers carries each scene's `scene.json`, and `ScenePointers::base` keeps its base
  parcel by hash, cleared with the pointers on a realm change. The mesh is one glTF container
  under the root, the url as its `src`, through the near LOD's container path. In place of a
  definition the root names the scene's hash (`StandInContent`), and a url resolves as itself
  under that hash with or without the scene's content map registered, so the download ranks in
  `Tier::Lod` under the scene's hash with the near LOD's. A far LOD costs one request, its
  mesh. It makes no colliders: its one container is a visuals part (`GltfPart::Visuals`), which
  builds none. It requests no variants manifest, and does not wait on its scene's: a url never
  has a variant.
- **A missing mesh is asked for once.** A definitive status for a far LOD's url (404, or another
  client error about the url itself: `failed_remotes::is_definitive`) is remembered for the
  session by scene, and the client offers no far LOD for that scene again
  (`FarLodSource::Unavailable`): leaving and re-entering the band sends no request. A scene with
  no mesh costs one request per session.
  The set is separate from `failed_remotes`: it is asked before the url is named, so a scene with
  no mesh takes no container and no far LOD place, while `failed_remotes` answers a read
  only when it is made and keeps a session status only for content reads. Only urls the client
  named count: a download of any other url that looks like a far LOD changes nothing. A name
  stays until its download answers; answered names are kept up to the bound `failed_remotes`
  keeps, the oldest forgotten past it, so naming never refuses and far LODs never stop for the
  session. The missing set has the same bound on its own: past it, a further missing mesh is not
  remembered and may be asked for again.
- **A breaker for an outage.** After five far LOD requests in a row fail with an error, a
  timeout or another status (`FAR_LOD_BREAKER_FAILURES`), far LODs pause for 60 s
  (`FAR_LOD_COOL_DOWN`). While paused they are still named, but no download starts
  (`IpfsIo::far_lod_may_request`, checked where the visuals start): those waiting keep their
  imposters and those shown stay, none stopped to make way. After the cool-down one far LOD
  may start, so at most one makes way for it: the probe, one per cool-down, and any answer, a mesh or a definitive status, resumes them. A
  probe served from the cache or dropped with its scene costs only the next cool-down. The
  client logs each trip and recovery once, and `/scene_lod_state` shows the missing count, the
  failures in a row and the seconds to the next probe (`far.missing`, `far.failures`,
  `far.paused_s`). A far LOD whose download failed with an error or a timeout shows its
  imposters until its scene leaves the band (the far ring or the tile edge) and comes back, or
  until far LODs resume after a pause. Then every far LOD whose mesh did not load, except a scene
  the server has none for, waits to start again where it is (`FarLodStatus::recoveries`): the
  far LODs that started before the breaker tripped all fail during an outage, which after a
  teleport can be most of those in view, not five.
- **Range in parcel rings.** It draws where the near LOD's keep band ends: its scene's nearest
  parcel past the near keep ring (4 by default, about 64 m) and within the far ring ("Far LOD
  Ring", default 20, about 320 m), the scene not wholly behind the camera. Nearer, it draws only
  to stand in for near visuals that find no place under the near LOD count, were stopped, or have
  nothing to show; past the near start ring, where near visuals cannot start, that is whenever
  they are not already shown, so a scene there (after a teleport, or once the count stopped its
  far LOD) is never left to its imposters. A far LOD stays, its visuals stopped, while its
  scene's near LOD shows, so when the near visuals stop (the setting lowered, or the player walks
  out) its mesh starts loading again at once, with no new far LOD to spawn and name; the scene
  shows its imposters only while that mesh loads. Beyond the far ring only the imposters remain.
  It has visuals only: no colliders part.
- **A count.** At most the far LOD count of far LODs (the far ring gives it, 200 at the default
  ring; none while Distant Scene Rendering is off)
  load or show at once, in view and behind the camera together, by the near LODs' visuals rule:
  nearest in view first while a place is free, then one behind the camera making way for each in
  view; the rest show their imposter tiles. Lowering it stops the farthest over it, those behind
  the camera first.
- **Edge on imposter tiles.** Imposters are tiles of 2^level parcels a side, and any tile with a
  stand-in's parcel in it is split down to single parcels. A far LOD is therefore shown only if
  every parcel of its scene within the scan ring lies where the imposters would draw a tile of level
  1 or less (`FAR_LOD_MAX_TILE_LEVEL`, 2 parcels a side). The imposters choose each tile's level
  through `Tiling::tile_distance` and `Tiling::level_allowed`, and the far LODs read the level
  through the same functions (`Tiling::level`), on the layout the imposters publish with their
  tiles (`ImposterTiling`, owned by the imposter plugin), so far LODs never draw over tiles laid
  out for another focus. Until the first layout (at login or after a realm change) no far LOD
  fits; that one is published as soon as the focus is known, without waiting for the player's
  scene to load, since there are no tiles yet to overlap. No larger tile ever holds a far LOD, so the edge
  between far LODs and imposters falls on a tile boundary and the splitting stays among the small
  tiles. At the default imposter distances that edge lies between about 200 m and 290 m; the far
  ring bounds it where the tiles reach further (a camera far from the player).
- **Which parcels.** A far LOD's parcels are those the realm's pointers name within the scan
  ring, which the pointers are read over: they decide whether it fits the tiles, bound its
  materials (`StandInBounds`) and are the parcels the imposters leave to it. A parcel past the
  scan ring is not checked. For a scene whose parcels are connected that changes nothing at the
  default rings and imposter distances with the imposters focused on the player: to reach past
  ring 20 the scene has a parcel in ring 20, at least 312 m away, whose 4-parcel tile starts at
  least 264 m away, past the 200 m where level 2 begins, so it fails the check there. That holds
  for a far ring of 16 or more. With a smaller far ring, a level-2 distance raised past what the
  far ring's parcels reach, disjoint parcels or a camera far from the player, a scene can show a
  far LOD with parcels past the scan ring; those lie outside its bounds, so the mesh is clipped
  there and the imposters draw them.
  A far LOD's parcels are fixed when it is spawned (a near LOD refreshes its own until its
  definition arrives). With the default distances they cannot go stale; with a raised level-2
  distance its clip bounds can lag until the far LOD is respawned.
- **No hysteresis at the edge.** A far LOD goes as soon as the focus parcel moves one of its
  parcels' tiles above level 1, and comes back when it moves back. Holding it any longer would
  split a level-2 tile around it, the cost the edge exists to avoid, so a player stepping back
  and forth across that parcel boundary sees the scene swap between mesh and imposter tile.
- **Cost per frame.** The scenes within the stand-ins' scan ring (ring 20 by default), with each
  one's nearest ring, parcels and whether its far LOD fits the tiles, are read from the pointers
  again only when the player's parcel, the scan ring, the realm's pointers
  (`ScenePointers::generation`), the preview state or the tile layout change; each frame checks
  only the stand-ins against what is on screen.
- **Memory.** The mesh keeps its CPU copy, like every mesh the container path loads: about
  0.5 MB per far LOD on average (sampled over 99 scenes within ring 20 of 0,0; the largest mesh
  holds 515k vertices), up to the far LOD count of them. No collision shape is built for it: the
  container path builds one only for a part that may make colliders.
- **Quantized meshes.** The meshes require `KHR_mesh_quantization` (16-bit positions, 8-bit
  normals, 16-bit texture coordinates). The vendored glTF loader reads them through a backport of
  upstream bevy's support (`rendering/bevy-explorer/docs/guidelines/bevy-fork.md`).
- **Hand-off.** Like the near LOD it shows in one frame once processed, and goes in the frame the
  live scene's content is on screen. Imposters skip its parcels while it is shown.

### What a loading scene shows

A loading scene raises no walls. It is parked below the world until it is ready and shows
nothing of its own; its stand-in covers its parcels instead: the near LOD where one is
shown, else the far LOD, else the imposter tile of its parcels, else nothing. The scene is
raised at its fifth tick, and its stand-in stays over it until the live scene's content is on
screen (`RendererSceneContext::content_shown`): the two overlap for those frames rather than
swapping in one. The walls' material, shader and systems are
gone; preview mode's grid plane under each parcel stays.

The walls never stopped a player from walking onto a loading scene. Within the near ring the near
LOD's colliders part does, drawn or not: a player who arrives first walks on the scene's collider
nodes, or on the floor where it has none.

### Settings

Scene loading is four bands around the player, each with one ring setting in parcels, under
"Scene Loading" in the Performance tab: live scenes, near LODs, far LODs and imposters. They are
read each frame, so a change applies on the next one, and are hidden in preview mode. The HUD's
Graphics Preset sets all four with "Distant Scene Rendering", so a preset is one profile (Medium
is the engine default); changing one shows the preset as Custom.

| Setting | Range | Low | Medium | High | What it sets |
| --- | --- | --- | --- | --- | --- |
| Live Scene Ring | 0 to 5 | 1 | 1 | 2 | scenes within it go live; kept one ring further |
| Near LOD Ring | 1 to 8 | 2 | 3 | 4 | near LODs start within it; kept one ring further |
| Far LOD Ring | 5 to 40 | 12 | 20 | 30 | far LODs' outer bound; the imposter tiles may end them sooner (about 25 to 30 parcels on Ultra) |
| Imposter Ring | 20 to 150 | 50 | 150 | 150 | where the imposter tiles end; 150 is no limit |
| Distant Scene Rendering (Graphics) | Off, Normal, Ultra | Normal | Normal | Ultra | the imposter tile distances; Off removes imposters and far LODs |

- **One control per band.** The ring is the only setting of its band. The counts follow it, so
  a wider ring never draws fewer: `AppConfig::near_lods_cap` is ring × (ring + 1), at most 40
  (6, 12 and 20 at rings 2, 3 and 4), and `AppConfig::far_lods_cap` is 100, 200 and 400 at rings
  12, 20 and 30, linear between and 600 at ring 40. They bound memory where scenes are dense,
  nearest in view first. The config's `near_lods_budget` and `far_lods_budget` replace them when
  set, for tests and measurements; no setting writes them.
- **The live ring replaces the load distance settings.** The live ring and its one-ring keep
  margin do what the load and unload distance sliders did, in the unit of the other bands. A
  saved config with a raised load distance moves once to the live ring that covers it (64 m to
  ring 4, at most 5), and both distances go back to their defaults.
- **The imposter ring ends the tile distances.** "Distant Scene Rendering" chooses the tile
  distances (Normal 100, 200, 400, 800, 1600 m and beyond; Ultra from 200 m) and the ring cuts
  the list at its distance (`AppConfig::set_imposters`), keeping the first so the level still
  reads back. At 150 nothing is cut.
- **Off.** "Distant Scene Rendering" Off removes the imposters and turns far LODs off: the far
  LOD count is 0 while there are no imposter distances, and at 0 no far LOD is spawned or kept.
  "Far LOD Ring" and "Imposter Ring" keep their values, which the panel shows disabled until it
  is turned on again, as "Unlimited Avatars" does to "Max Avatars". Live scenes and near LODs
  are unchanged.

The rings stay ordered whatever the values. A config value outside a range is clamped to it.
Far LODs start one ring past the near keep ring, or past the live keep ring when the live ring
reaches further. A near ring at or inside the live ring leaves near LODs only to live scenes
still loading. A far ring inside the near keep ring leaves no normal far-start band; far LODs
may still stand in for near LODs that wait over the near LOD count or have nothing to
show. The Scene stand-ins debug section and `/scene_lod_state` report the active rings.

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
- **Far LODs out to ring 20 regardless of the tiles.** Every 4×4 tile between 200 m and 320 m
  touching a far LOD would be split into single-parcel imposters, each its own load, and a tile
  swap shows the old tile over the LOD until its children load.
- **Far LODs cut off at the tiles' parcels.** Scene materials can clip to parcels, but a scene
  on the edge would show part mesh and part imposter.
- **A near-LOD range in metres.** Rings are what the partition, the download order and the
  waves already measure, and a keep ring one wider than the start ring stops a player on the
  edge from starting and dropping a stand-in every few steps.

## Consequences

- A live scene no longer respawns at the load edge: the unload distance and the keep ring are
  real margins.
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
- A loading scene shows its stand-in instead of animated walls.
- Within three rings, a loading scene shows its own models as soon as they arrive, and the live
  scene reuses those models when it comes up.
- A scene the asset pipeline has not processed has no descriptor and no LOD: its imposter tile
  stands in where one is baked, and nothing shows on its parcels where none is.
- A stand-in's collisions are the defaults only: a container's collision masks, `MeshCollider`
  components set by scene code, primitives a script creates and things a script moves are
  absent until the scene is live.
- A stand-in shows only content-file glTFs. Url models, primitives, text and code-set materials
  appear when the scene is live; the primitives, text and materials need the descriptor to
  describe them.
- Out to the edge of the small imposter tiles (about 200 m to 290 m at the default imposter
  distances), a scene the pipeline has meshed shows that mesh where it would have shown its
  imposter tile.
- Scenes the pipeline has not yet given a Bevy copy show their imposters, not the far LOD,
  until it does.
- A far LOD costs one request, its mesh. A scene the server has no mesh for costs that request
  once a session, and a failing LOD service a few requests a minute.
- A large scene with a parcel within the scan ring outside the small tiles has no far LOD while
  that parcel is there.
