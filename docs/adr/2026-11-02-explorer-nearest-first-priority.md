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

The Bevy Explorer had none of this order. The scene loop ranked a scene by the squared distance
from the player to its base parcel divided by the time since it was last sent, with no facing
term, so a scene behind the player cost as much as one in front. Downloads took the ipfs crate's
request slots, a first-come-first-served `tokio` semaphore, so the first scene to ask filled the
queue whatever its distance. Spawning is unordered too: `process_scene_lifecycle` spawns the
scenes that come into range in hash-map order, in one frame, and this change leaves it so (see
Rejected). The parity work (card E2.priority, split from E2, W1 and L1 for their merged leaf
task) set out to close the tick and download gap. Two constraints shaped how: implement it
closely to unity-explorer, and use no buckets, since the aging scheduler already shares ticks by
distance and a table of steps is a second source of tuning.

## Decision

Scene ticks and download slots order by one fact about a scene, or an avatar, its
**partition**, and every partition is measured the same way.

- **One measurement, two uses.** `common::partition::scene_partition` turns the player's
  position, the rendering camera's position and forward, and a scene's parcels into
  `PartitionData`, on the ground plane: the squared distance from the player to the nearest
  point of the scene's nearest parcel, the smallest Chebyshev distance in parcels from the
  player's parcel to any of the scene's parcels, and whether every corner of every parcel is
  behind the camera (unity-explorer's test). A scene with no parcels (a portable, a global scene)
  is nearest and in front. It is a component, `Partition`, on every scene root that has a
  `RendererSceneContext`, refreshed every frame by `update_scene_partitions` from the context's
  parcels; `update_scene_priority` and `publish_download_priorities` read it. The load range
  (`parcels_in_range`) measures for itself, from the player, with the same nearest-point metric.
- **Avatars are partitioned too, as points, and their wearables inherit it.** Every entity with
  `PartitionedDownloads`, the content hashes its downloads are filed under, gets a `Partition`
  from its position (`point_partition`: distance from the player, the ring of its parcel, behind
  the camera or not). The avatar crate (`update_render_avatar`) puts the entity hashes of the
  body, the wearables and the default wearables there as their pointers resolve, so a
  wearable's definition, model and textures queue in the avatar tier at the avatar's partition,
  and a wearable several avatars share at the best of theirs. Every avatar comes after the
  player's scenes and before any other scene; among avatars the key decides, nearer ring first,
  in front before behind within a ring, then distance. Scene content stays per scene (the
  download-slot entry below); avatars are the exception, and unity-explorer partitions them
  per entity too.
- **A tier, then raw distance and parcel distance, no buckets.** The admission key is
  `LoadPriority (tier, parcel_distance, is_behind, sqr_distance)`, lower first. The tier is the
  band a download belongs to: the player's scenes, then avatars, then the other scenes, then
  requests that are no scene's and no avatar's, then scenes that have left. Within a band the
  partition decides; parcel distance comes before facing only so that "in front before behind"
  can still decide between scenes at the same parcel ring, since raw squared distance alone never
  ties. The tick key uses raw squared distance. unity-explorer's bucket thresholds and its
  tick-rate table are not ported.
- **The nearest point of the parcel rectangle, not its corner.** The distance is 0 when the
  camera is over a scene, and it is the measure `parcels_in_range` already uses.
- **Distance from the player, facing from the rendering camera.** Distance is measured from
  the player, as the tick key always was. Facing comes from the `PrimaryCamera` that has a
  `Camera`; in third person that is a few metres behind the player, and often inside a
  neighbouring scene. Without a rendering camera, as on the headless server, whose stand-in
  `PrimaryCamera` has no `Camera` and never turns, no facing is known and nothing is behind.
- **Behind the camera means four times farther in the tick queue.** The scene loop keeps its
  aging key, squared distance over time since last sent, now measured to the nearest parcel of
  the whole scene instead of its base parcel, and multiplies it by `BEHIND_SCENE_TICK_PENALTY`
  (4) for a scene behind the camera. While frame time is spare nothing changes: every eligible
  scene is sent. When it is short, behind scenes yield first and still tick, less often. The
  player's scenes (the one they stand in, plus portables and global scenes) keep the key 0.
- **Download slots go by the key, not by arrival.** The ipfs crate's request slots are a
  `PriorityGate` instead of a first-come semaphore. Requests queue per owning entity hash (a
  scene's, a wearable's), plus one queue for requests that carry no hash; a freed slot, or
  one added when the slot count rises, pops the head of the best-ranked queue, best
  `LoadPriority` first and the oldest head on ties. So a grant costs a scan over the queues, one
  per hash with a request waiting (500 in the benchmark, with 5,000 requests waiting), not over
  the thousands of requests waiting during an initial load, and a key change re-ranks queues,
  never requests. The request's hash is read from the asset path (`IpfsPath::scene_hash`: the
  scene of a content file or scene content, or an entity definition's own hash), so no loader
  changes. A reader that joins a download already in flight queues the same ticket under its own
  scene; the download is granted once, from whichever queue reaches it first, so it rises with
  the nearest of its readers. Admission is per scene (or avatar), and one gate covers every
  asset kind that downloads through `IpfsIo`; disk-cache hits, `file://` realm reads and static
  files take no slot.
- **Unknown hash is top; no hash is after the live scenes; departed is last.** A request whose
  path carries a hash nobody has published yet (a scene or an avatar in its first frame, an
  emote) keeps today's place, the top. A request with no hash at all (NFT images, URL-form fonts
  and profile pictures, the imposter archives behind `async_request`) is in the `Other` tier,
  after every live scene and avatar: an NFT frame or an imposter must not beat the active scene's
  files. A hash that was published and no longer is, because its scene despawned or its avatar
  dropped it, is in the `Departed` tier, last, for as long as the requests already queued under
  it wait. The mark lives with the queue: a request that finds no queue waiting for a departed
  hash is unknown again, and on top.
- **Keys cross the crate boundary as data.** `publish_download_priorities` writes `hash →
  LoadPriority` into `IpfsIo` (`set_scene_priorities`) on every frame in which a download waits
  for a slot, because `ipfs` cannot depend on `scene_runner`: the player's scenes at
  `LoadPriority::TOP`, the other scenes at their partition in the `Scene` tier, and each avatar's
  wearable hashes at the avatar's partition in the `Avatar` tier, the best of them for a shared
  one. The gate keeps the map and ranks its queues by it at each grant; publishing touches no
  request, and an idle frame publishes nothing, since the keys are only read at a grant. That
  map is the one per-scene (and per-avatar) fact this change keeps outside the entity, and it is
  a cache the gate reads, not state systems branch on.
- **Constants, not settings, and no sampling tolerance.** `BEHIND_SCENE_TICK_PENALTY` and the
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
  `ScenesPartitioningUtils`). It reads 11.3 m from a point over the centre of a parcel and is 0
  only on a corner, and it would disagree with `parcels_in_range`, which decides what is in range
  by the nearest point.
- **A once-a-second tick cap for scenes behind the camera** (unity-explorer's `behindFps = 1`).
  A scene sent once a second delivers a second's worth of changes in one frame: lumpy engine work
  that hitches. The scheduler already skips ticks when frame time runs short, so a penalty on
  the key degrades smoothly where a cap degrades in steps.
- **Sorting the spawn order.** Every newly required scene spawns in the same frame and
  `load_scene_entity` starts all their definition loads in that frame, so the sort only decided
  which small definition download queued first, `defer_to_current_scene` already puts the
  player's scene first, and the partition it needed came from in-range pointer parcels, which
  can disagree with the scene's parcels once it has a context. It returns with W1's loading
  waves, which take the first N of a sorted list.
- **Measuring distance from the camera.** In third person the camera often sits inside a
  neighbouring scene, whose distance then reads 0 and ties with the active scene's key of 0; the
  stable sort leaves query order to decide. The camera gives facing only.
- **Ranking every waiting request on each grant, and re-scoring them all every frame.** Linear in
  requests, which reach thousands during an initial load in a dense area; the per-hash queues
  make a grant and a key change linear in the queues (scenes and wearables with a request
  waiting) instead.
- **Per-entity partition of scene content within 48 m, and an admission stage in front of each
  loader.** unity-explorer re-partitions each loadable entity within `fastPathDistance` of the
  camera (48 m as shipped, `PartitionAssetEntitiesSystem`); the Explorer's loaders have no
  admission stage to feed, and one request-slot gate covers every asset kind. Avatars are the one
  per-entity partition kept, because their downloads carry their own hashes, not a scene's.
- **Requests without a hash at the top.** The first-come behaviour before this change let an NFT
  frame or an imposter archive queue ahead of the active scene's files; they rank after every
  live scene instead.
- **Avatars interleaved with scenes by distance.** Ranking an avatar's wearables by the same key
  as scenes puts an avatar five parcels away behind a scene one parcel away; every avatar is
  wanted before the non-active scenes, so avatars are a tier of their own, distance deciding
  within it.
- **A flat avatar tier, first come first served.** Distance order among avatars is wanted: the
  nearest avatar's wearables first. It costs a partition per avatar and a map entry per wearable
  hash on every frame with a download waiting: for 20 live scenes and 50 avatars with 10
  wearables each (353 distinct), partitioning, building and publishing the map took about 300 µs
  a frame in the dev profile, and about 30 µs on a frame with nothing waiting, when only the
  partitions are refreshed (`overhead_of_publishing_at_a_busy_plaza`).
- **Keeping the first-come semaphore for download slots.** The first scene to ask fills the queue
  whatever its distance; the gate decides at every grant instead, as unity-explorer re-sorts its
  pending loads every frame.
- **A camera sampling tolerance** (re-measure only after the camera moves 0.1 m or turns 3°,
  `PartitionSettings.asset`). Tens of scenes times their parcels, plus tens of avatars, is a few
  thousand operations a frame; the tolerance amortises a Burst job and a world re-sort that the
  Explorer does not have.
- **Facing without a rendering camera.** Neither the headless stand-in `PrimaryCamera`, which
  never turns, nor the player's own forward stands for a viewer: a behind flag from either would
  count a server's scenes on one side (south, for the stand-in) four times farther in the tick
  queue while nobody looks.
- **Treating a departed scene's queued downloads as unknown.** Unknown is top, so a scene just
  despawned would be served ahead of the scenes in front; its queue keeps the bottom key instead.
- **User-facing settings for the penalty and the key.** unity-explorer keeps its equivalents as
  asset values with no runtime setter; here they are code constants.
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

- A scene the player faces gets its files and its ticks before a scene at the same distance
  behind them; turning round reorders the tick and download queues within a frame.
- Strict priority can hold a far scene's downloads while nearer scenes keep requesting. That is
  unity-explorer's behaviour too, and the load distance bounds how many scenes compete. The
  avatar tier is strict as well: while any avatar's wearables wait, no scene but the player's
  gets a slot, so a crowd delays the neighbouring scenes' files by its wearables.
- A scene behind the camera ticks as before while frame time is spare, and is the first to tick
  less often when it is not; the player's scenes never yield.
- NFT images, profile pictures, Fontsource font families and imposter archives name no scene,
  so they download after every live scene's and avatar's files, however near the NFT frame is. A
  texture or font a scene gives as a URL goes through its content-file path and takes its key;
  JavaScript `fetch` takes no request slot at all.
- On the headless server nothing is behind, so no scene's tick key is penalised; its scenes
  order by distance from the player only.

## Follow-ups

- Carry the scene on URL-form asset paths (NFT images, Fontsource fonts) so they take their
  scene's key instead of the flat rank after the live scenes.
- Emotes load under their own entity hashes like wearables; filing them under the avatar's
  partition is the same mechanism, not done here.
- Loading waves (W1) sort the newly required scenes by their partition and take the first N;
  until then spawn order is hash-map order.
- `scene_timing` (`scene_runner::load_timing`) logs, per scene, the time from spawn to its fifth
  tick and, per arrival, the time until the player's scene and its first ring are ready. It is
  instrumentation, not part of the decision: run it on `main` (the commit that adds it applies
  there alone) and on this branch to see whether the order shortens the player's load.
