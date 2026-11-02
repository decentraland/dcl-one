# ADR: The Explorer orders scenes and downloads from the camera, nearest first, without distance buckets

- Status: Accepted
- Date: 2026-11-02

## Context

unity-explorer orders everything that costs time by where the camera is and where it looks.
Scenes load in front of the camera first, the player's scene first, then nearest first
(`OrdenedDataNativeComparer`); a scene's JavaScript ticks at a rate set by its distance, and a
scene behind the camera at most once a second (`GetSceneUpdateFrequency`, `behindFps = 1`); each
scene world admits its pending asset loads of every kind by distance, in front before behind,
against a shared loading budget (`DeferredLoadingSystem`, `BucketBasedComparer`). Its unit of
distance is a bucket: thresholds of 16 m steps up to 160 m, then 320, 640 and 1,280 m, compared
squared against the distance to the scene's nearest parcel corner. The bucket indexes a tick-rate
table of 40, 30, 15, 10, 5 and 0 frames a second, so a scene from 80 m on is frozen. These are the
shipped values (`PartitionSettings.asset`, `RealmPartitionSettings.asset`); the code defaults
differ.

The Bevy Explorer had none of this order. `process_scene_lifecycle` spawned the scenes that came
into range in hash-map order, in one frame. The scene loop ranked a scene by the squared distance
from the player to its base parcel divided by the time since it was last sent, with no facing
term, so a scene behind the player cost as much as one in front. Downloads took the ipfs crate's
request slots, a first-come-first-served `tokio` semaphore, so the first scene to ask filled the
queue whatever its distance. The parity work (card E2.priority, split from E2, W1 and L1 for
their merged leaf task) set out to close this gap. Two constraints shaped how: implement it
closely to unity-explorer, and use no buckets, since the aging scheduler already shares ticks by
distance and a table of steps is a second source of tuning.

## Decision

Scene spawning, scene ticks and download slots order by one fact about a scene, its
**partition**, and every partition is measured the same way.

- **One measurement, three uses.** `common::partition::scene_partition` turns the camera's
  position and forward and a scene's parcels into `ScenePartitionData`, on the ground plane: the
  squared distance from the camera to the nearest point of the scene's nearest parcel, the
  smallest Chebyshev distance in parcels from the camera's parcel to any of the scene's parcels,
  and whether every corner of every parcel is behind the camera (unity-explorer's test). A scene
  with no parcels (a portable, a global scene) is nearest and in front. It is a component,
  `ScenePartition`, on every scene root from the moment the lifecycle spawns it, refreshed every
  frame: by the lifecycle from the scene's in-range parcels in the pointers until the scene has a
  `RendererSceneContext`, then by `update_scene_partitions` from the context's parcels.
  `update_scene_priority` and `publish_download_priorities` read the component, and the lifecycle
  sorts its spawns with the same function. Only the load range (`parcels_in_range`) still
  measures for itself, from the player, with the same nearest-point metric.
- **Raw distance and parcel distance, no buckets.** The admission key is
  `LoadPriority (parcel_distance, is_behind, sqr_distance)`, lower first. Parcel distance comes
  first only so that "in front before behind" can still decide between scenes at the same parcel
  ring: raw squared distance alone never ties, and the facing term would be dead. Spawn order and
  the tick key use raw squared distance. unity-explorer's bucket thresholds and its tick-rate
  table are not ported.
- **The nearest point of the parcel rectangle, not its corner.** The distance is 0 when the
  camera is over a scene, and it is the measure `parcels_in_range` already uses.
- **Facing comes from a rendering camera only.** Partitions are measured from the
  `PrimaryCamera` that has a `Camera`, as unity-explorer samples its camera; in third person that
  is a few metres behind the player. Without one, as on the headless server, whose stand-in
  `PrimaryCamera` has no `Camera` and never turns, partitions use the player's position and a
  zero forward: nothing is behind.
- **Spawn order follows unity-explorer's loading comparer.** Newly required scenes are sorted by
  `(is_behind, !player_inside, sqr_distance, lowest parcel)`: in front of the camera before
  behind, the player's scene first, then nearest first, then the scene's lowest in-range parcel
  (x, then y) as a deterministic tiebreak where unity-explorer uses the base parcel's x, which a
  pointer does not carry. They still all spawn in the one frame, and their `LoadSceneEvent`s go
  out in that order. Portables and system scenes keep their exemptions from the startup and
  current-scene deferrals.
- **Behind the camera means at most once a second, as in unity-explorer.** The scene loop keeps
  its aging key, squared distance over time since last sent, now measured from the camera to the
  nearest parcel of the whole scene instead of from the player to its base parcel, and does not
  queue a scene behind the camera until a second has passed since it was last sent
  (`BEHIND_SCENE_TICK_INTERVAL`). The player's scenes (the one they stand in, plus portables and
  global scenes) and system scenes (`SuperUserScene`) are never held back.
- **Download slots go by the key, not by arrival.** The ipfs crate's request slots are a
  `PriorityGate` instead of a first-come semaphore. A freed slot, or one added when the slot count
  rises, goes to the waiter with the best `LoadPriority` at that moment, oldest first on ties; a
  newcomer takes a free slot directly only when nobody waits. The waiter's scene is read from the
  asset path (`IpfsPath::scene_hash`: the scene of a content file or scene content, or an entity
  definition's own hash), so no loader changes. A reader that joins a download already in flight
  adds its scene to it, and a download for several scenes takes the best of their keys, so one
  still waiting for a slot rises with the nearer reader. Admission is per scene, and one gate
  covers every asset kind that downloads through `IpfsIo`; disk-cache hits take no slot.
- **Unknown is top, departed is bottom.** A waiter whose path names no published scene keeps
  today's place, the top: requests that are not a scene's (wearables and emotes, addressed by
  their own entity hash; profile pictures and other URL-form paths; imposters'
  `async_request`) and a scene not yet published. A scene that was published and no longer is,
  because it was despawned, keeps `LoadPriority::BOTTOM` for as long as a waiter names it.
- **Keys cross the crate boundary as data.** `publish_download_priorities` writes `scene hash →
  LoadPriority` into `IpfsIo` (`set_scene_priorities`) every frame, the player's scenes at
  `LoadPriority::TOP`, because `ipfs` cannot depend on `scene_runner`. That map is the one
  per-scene fact this change keeps outside the scene's entity, and it is a cache the gate reads,
  not state systems branch on.
- **Constants, not settings, and no sampling tolerance.** `BEHIND_SCENE_TICK_INTERVAL` and the
  key's shape are code constants, as unity-explorer's are asset values; partitions are recomputed
  every frame from the camera as it is.

## Rejected

- **Distance buckets and a bucket-indexed tick-rate table.** Excluded by the constraint in
  Context: the aging scheduler already shares ticks by distance, and ported as shipped the table
  would also freeze every scene from 80 m on. No bucket field in `LoadPriority`, no rate table in
  the scene loop.
- **Raw squared distance first, facing second** (unity-explorer's `DistanceBasedComparer`, whose
  "bucket" is a float). A float distance never ties, so facing never decides; the integer parcel
  ring in front of it is what keeps facing alive.
- **Parcel distance as `floor(distance / 16)`.** The ring is the Chebyshev ring index, the square
  rings unity-explorer loads by (`ParcelMathJobifiedHelper`), not a rounding of the metric
  distance.
- **The nearest parcel corner as the distance** (unity-explorer's measure,
  `ScenesPartitioningUtils`). It reads 11.3 m at the centre of a parcel the camera stands over and
  is 0 only on a corner, and it would disagree with `parcels_in_range`, which decides what is in
  range by the nearest point.
- **A priority multiplier for scenes behind the camera.** It keeps every behind scene ticking at
  a lower share instead of capping it; unity-explorer ships the cap (`behindFps = 1`), and the cap
  is what is ported.
- **Per-entity partition within 48 m, and an admission stage in front of each loader.**
  unity-explorer re-partitions each loadable entity within `fastPathDistance` of the camera (48 m
  as shipped, `PartitionAssetEntitiesSystem`); the Explorer's loaders have no admission stage to
  feed, and one per-scene gate covers every asset kind.
- **Keeping the first-come semaphore for download slots.** The first scene to ask fills the queue
  whatever its distance; the gate decides at every grant instead, as unity-explorer re-sorts its
  pending loads every frame.
- **A camera sampling tolerance** (re-measure only after the camera moves 0.1 m or turns 3°,
  `PartitionSettings.asset`). Tens of scenes times their parcels is a few thousand operations a
  frame; the tolerance amortises a Burst job and a world re-sort that the Explorer does not have.
- **Facing without a rendering camera.** Neither the headless stand-in `PrimaryCamera`, which
  never turns, nor the player's own forward stands for a viewer: a behind flag from either would
  tick a server's scenes on one side (south, for the stand-in) at one hertz while nobody looks.
- **Treating a departed scene's queued downloads as unknown.** Unknown is top, so a scene just
  despawned would be served ahead of the scenes in front; it keeps the bottom key instead.
- **User-facing settings for the interval and the key.** unity-explorer keeps them as asset
  values with no runtime setter; here they are code constants.
- **Changing the GPU transfer priority with this.** `RenderAssetTransferPriority` stays as it is
  (`Priority(1)` for the scene an out-of-world player is arriving in, `0` otherwise); it orders
  uploads only under the per-frame byte budget, which is on by default only on the web
  (`gpu_bytes_per_frame`).
- **Per-scene priority in a resource map.** A fact about a scene is a component on its root; the
  gate's map inside `IpfsIo` is the one exception, because `ipfs` cannot depend on
  `scene_runner`.
- **Loading waves, budgets, LOD and the unload margin in this change.** Loading waves (six scenes
  a wave, `ScenesRequestBatchSize`) and the unload margin belong to card W1; frame-time,
  loading-slot and memory budgets to card E2, which builds on the memory status provider
  (W6.provider); scene LOD and the visual-state resolver to W4.

## Consequences

- A scene the player faces gets its files, its first ticks and its spawn before a scene at the
  same distance behind them; turning round reorders the tick and download queues within a frame.
- Strict priority can hold a far scene's downloads while nearer scenes keep requesting. That is
  unity-explorer's behaviour too, and the load distance bounds how many scenes compete.
- A scene behind the camera populates its entities at one hertz until the player turns to it;
  tweens and animations are engine-side and unaffected, scripted movement is not.
- NFT images and Fontsource font families load through URL-form paths (`IpfsPath::new_from_url`)
  that name no scene, so they stay top priority and a far scene's NFT frames compete with near
  scenes' content files. A texture or font a scene gives as a URL goes through its content-file
  path and takes its key; JavaScript `fetch` takes no request slot at all.
- On the headless server nothing is behind and no tick is capped; its scenes order by distance
  from the player only.

## Follow-ups

- Carry the scene on URL-form asset paths (NFT images, Fontsource fonts) so they take their
  scene's key.
- Loading waves (W1) take the first N of the lifecycle's sorted list.
- The gate's `set_priorities` and `grant` are linear in the waiters; revisit if download queues
  grow past a few hundred.
