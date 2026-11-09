# ADR: The Explorer orders scenes and downloads from the camera, nearest first, without distance buckets

- Status: Accepted
- Date: 2026-11-02

## Context

The scene loop and download gate must prioritize the player's current scene and nearby visible
content. Both need consistent distance and facing measurements. The aging scheduler already
shares scene ticks by distance, so adding a bucket-indexed tick-rate table would introduce a
second source of tuning.

A scene's base parcel alone does not describe its distance when it spans several parcels.
Arrival-ordered download slots can also let a distant scene delay the active scene's files.
This decision gives ticks and downloads one partition model. Spawning remains unordered:
`process_scene_lifecycle` starts every newly required scene in the same frame, and sorting those
spawns without a loading-wave limit would only reorder their initial definition requests.

## Decision

Scene ticks and download slots order by one fact about a scene, or an avatar, its
**partition**, and every partition is measured the same way.

- **One measurement, two uses.** `common::partition::scene_partition` turns the player's
  position, the rendering camera's position and forward, and a scene's parcels into
  `PartitionData`, on the ground plane: the squared distance from the player to the nearest
  point of the scene's nearest parcel, the smallest Chebyshev distance in parcels from the
  player's parcel to any of the scene's parcels, and whether every corner of every parcel is
  behind the camera. A scene with no parcels (a portable, a global scene)
  is nearest and in front. It is a component, `Partition`, on every scene root that has a
  `RendererSceneContext`, refreshed every frame by `update_scene_partitions` from the context's
  parcels; `update_scene_priority` and `publish_download_priorities` read it. The load range
  (`parcels_in_range`) measures for itself, from the player, with the same nearest-point metric.
- **Avatars are partitioned too, as points, and their wearables inherit it.** Every entity with
  `PartitionedDownloads`, the content hashes its downloads are filed under, gets a `Partition`
  from its position (`point_partition`: distance from the player, the ring of its parcel, behind
  the camera or not). The avatar crate (`update_render_avatar`) puts the entity hashes of the
  body, the wearables and the default wearables there as their pointers resolve, and only when
  the list changes, so a wearable's definition, model and textures queue in the avatar tier at
  the avatar's partition, and a wearable several avatars share at the best of theirs. Every
  avatar comes after the player's scenes and before any other scene; among avatars the key
  decides, nearer ring first, in front before behind within a ring, then distance. Scene content
  stays per scene (the download-slot entry below); avatar downloads are partitioned per entity.
- **A tier, then raw distance and parcel distance, no buckets.** The admission key is
  `LoadPriority (tier, parcel_distance, is_behind, sqr_distance)`, lower first. The tier is the
  band a download belongs to: the player's scenes, then avatars, then the other scenes, then
  requests whose hash no scene root or avatar holds, then scenes that have left. Within the avatar
  and scene bands the partition decides, and in the others the oldest request goes first; parcel
  distance comes before facing only so that "in front before behind" can still decide between
  scenes at the same parcel ring, since a raw-distance key gives facing no say between unequal
  distances. The tick key uses
  raw squared distance. No distance bucket thresholds or tick-rate table are used.
- **The nearest point of the parcel rectangle, not its corner.** The distance is 0 when the
  player stands within a scene's parcels, and it is the measure `parcels_in_range` already uses.
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
- **The entity that owns a hash registers it; unregistered and no-hash requests come after
  the live scenes; departed is last.** `SceneHash` and `PartitionedDownloads` carry component
  hooks: inserting them registers their hashes with the request slots
  (`IpfsIo::register_download_priority`), and replacing or removing them, on a despawn, an
  outfit change, a hash replaced or an avatar selection removed (a scene drops an entity's
  `AvatarShape` but keeps the entity; `update_render_avatar` drops the component with it),
  releases them (`release_download_priority`). The gate counts
  a hash's holders, so a wearable two avatars share stays while one of them leaves, and a key
  goes only with its last holder. A scene's hash is registered at spawn, at the bottom of the
  scene tier, and the player's scene is raised to the top by the first publish with a download
  waiting, before it has a context, so nothing has to rank above the registered hashes by
  default: a request whose hash nobody holds (an avatar's emote, a backpack thumbnail)
  ranks with the requests that carry no hash at all (NFT images, URL-form fonts and profile
  pictures, the imposter archives behind `IpfsIo::fetch_request`), in the `Other` tier after every
  registered scene and avatar, oldest first. A hash released by its last holder while a request
  under it is alive (waiting, in an attempt or between attempts), because its scene despawned or
  its avatar dropped it, is in the `Departed` tier, last, for as long as such a request is alive;
  the mark lives with the hash's live requests, so their retries and a request for the hash that
  arrives meanwhile rank last too, and one that arrives after the last of them has ended is
  unregistered again, in `Other`. Registering the hash again ends the mark.
- **Keys cross the crate boundary as data, written in place.** Which hashes have a key is the
  hooks' business (above); their values are `publish_download_priorities`'s, which on every
  frame in which a download waits for or holds a slot writes this frame's key of every registered hash
  into `IpfsIo` (`update_download_priorities`), because `ipfs` cannot depend on `scene_runner`:
  the player's scenes at `LoadPriority::TOP` (found through `ContainingScene::get_oow`, so a
  scene without a context yet counts, and so does the destination while the player is still
  out of world arriving in it), the other scenes at their partition in the `Scene` tier
  (`SCENE_PENDING`, the tier's bottom, until a scene has one), and each avatar's wearable
  hashes at the avatar's partition in the `Avatar` tier, the best of them for a shared one (the
  first write of a hash in a publish replaces its key, later writes take the best, and a hash
  nobody registered is ignored). The gate owns the keys and ranks its queues by them at each
  grant; publishing builds no map, clones no string and touches no request, and an idle frame
  publishes nothing, since the keys are only read at a grant: while idle only values can go
  stale, never which hashes are held. The keys are the one per-scene (and per-avatar) fact this
  change keeps outside the entity, an index that follows the components' lifecycle, not state
  systems branch on.
- **Constants, not settings, and no sampling tolerance.** `BEHIND_SCENE_TICK_PENALTY` and the
  key's shape are code constants; partitions are recomputed every frame from the camera as it is.

## Rejected

- **Distance buckets and a bucket-indexed tick-rate table.** Excluded by the constraint in
  Context: the aging scheduler already shares ticks by distance. The evaluated alternative
  uses 16 m steps up to 160 m, then 320, 640 and 1,280 m, measured from the nearest parcel corner,
  and a tick-rate table of 40, 30, 15, 10, 5 and 0 frames a second. That table freezes scenes
  from 80 m on and adds a second tuning policy. No bucket field in `LoadPriority`, no rate
  table in the scene loop.
- **Raw squared distance first, facing second.** Facing could then break only exact-distance
  ties: it could never put a slightly farther scene in front ahead of a nearer scene behind.
  An integer parcel ring groups those scenes so facing can decide within the ring.
- **Parcel distance as `floor(distance / 16)`.** The ring is the Chebyshev ring index,
  which measures square parcel rings rather than rounding the metric distance.
- **The nearest parcel corner as the distance.** It reads 11.3 m from a point over the
  centre of a parcel and is 0 only on a corner. It would disagree with `parcels_in_range`, which
  decides what is in range
  by the nearest point.
- **A once-a-second tick cap for scenes behind the camera.**
  A scene sent once a second delivers a second's worth of changes in one frame: lumpy engine work
  that hitches. The scheduler already skips ticks when frame time runs short, so a penalty on
  the key degrades smoothly where a cap degrades in steps.
- **Sorting the spawn order.** Every newly required scene spawns in the same frame and
  `load_scene_entity` starts all their definition loads in that frame, so the sort only decided
  which small definition download queued first, `defer_to_current_scene` already puts the
  player's scene first, and the partition it needed came from in-range pointer parcels, which
  can disagree with the scene's parcels once it has a context. It can be revisited with loading
  waves, which take the first N of a sorted list.
- **Measuring distance from the camera.** In third person the camera often sits inside a
  neighbouring scene, whose distance then reads 0 and ties with the active scene's key of 0; the
  stable sort leaves query order to decide. The camera gives facing only.
- **Ranking every waiting request on each grant, and re-scoring them all every frame.** Linear in
  requests, which reach thousands during an initial load in a dense area; the per-hash queues
  make a grant and a key change linear in the queues (scenes and wearables with a request
  waiting) instead.
- **Rebuilding the key map every frame and handing it over** (this change's first shape). A
  fresh `HashMap<String, LoadPriority>` on every frame with a download waiting, a string clone
  per scene and per wearable, the old map freed, and departure inferred from a hash going
  missing between two publishes, with a `published` flag per queue. The hooks keep which hashes
  are held exact at every moment, on frames the publish skips too, cleanup comes with the
  components' lifecycle, and the publish writes values in place, with no map and no string clone.
- **An unregistered hash at the top.** It was needed only while the player's scene had no key
  until it had a context, and it let every new scene's entity definition and `main.crdt`,
  avatars' emotes and backpack thumbnails tie with the player's scene. Registering `SceneHash`
  at spawn and keying the player's scene before its context removes the need.
- **Per-entity partition of scene content within 48 m, and an admission stage in front of each
  loader.** The evaluated alternative re-partitions every loadable entity within 48 m of the
  camera. The Explorer's loaders have no admission stage to feed, and one request-slot gate
  covers every asset kind. Avatars are the one per-entity partition kept, because their
  downloads carry their own hashes, not a scene's.
- **Requests without a hash at the top.** The first-come behaviour before this change let an NFT
  frame or an imposter archive queue ahead of the active scene's files; they rank after every
  live scene instead.
- **Avatars interleaved with scenes by distance.** Ranking an avatar's wearables by the same key
  as scenes puts an avatar five parcels away behind a scene one parcel away; every avatar is
  wanted before the non-active scenes, so avatars are a tier of their own, distance deciding
  within it.
- **A flat avatar tier, first come first served.** Distance order among avatars is wanted: the
  nearest avatar's wearables first. It costs a partition per avatar and a key written per
  wearable hash on every frame with a download waiting: for 20 live scenes and 50 avatars with
  10 wearables each (353 distinct), partitioning and writing the keys took about 115 µs a frame
  in the dev profile, and about 25 µs on a frame with nothing waiting, when only the partitions
  are refreshed (`overhead_of_publishing_at_a_busy_plaza`).
- **Keeping the first-come semaphore for download slots.** The first scene to ask fills the queue
  whatever its distance; the gate decides using current priorities at every grant instead.
- **A camera sampling tolerance** (re-measure only after the camera moves 0.1 m or turns 3°).
  Tens of scenes times their parcels, plus tens of avatars, is a few thousand operations a
  frame. The evaluated tolerance amortizes a global partition-and-sort pipeline; this engine
  updates per-entity partition values without that global pipeline, so it recomputes them every
  frame. The tick scheduler still sorts the due-scene queue each frame.
- **Facing without a rendering camera.** Neither the headless stand-in `PrimaryCamera`, which
  never turns, nor the player's own forward stands for a viewer: a behind flag from either would
  count a server's scenes on one side (south, for the stand-in) four times farther in the tick
  queue while nobody looks.
- **Treating a departed scene's queued downloads as unregistered.** They would rank in `Other`,
  level with NFT frames and imposters, instead of last; the hash is marked departed when its
  last holder releases it while one of its requests is alive.
- **User-facing settings for the penalty and the key.** They remain code constants.
- **Changing the GPU transfer priority with this.** `RenderAssetTransferPriority` stays as it is
  (`Priority(1)` for the scene an out-of-world player is arriving in, `0` otherwise); it orders
  uploads only under the per-frame byte budget, which is on by default only on the web
  (`gpu_bytes_per_frame`).
- **Per-scene priority in a resource map.** A fact about a scene is a component on its root; the
  gate's keys inside `IpfsIo` are the one exception, because `ipfs` cannot depend on
  `scene_runner`, and the hooks keep them in step with the components.
- **Loading waves, budgets, LOD and the unload margin in this change.** Loading waves (six
  scenes a wave) and the unload margin are separate scene-loading work. Frame-time,
  loading-slot and memory budgets build on the memory status provider; scene LOD and the
  visual-state resolver are separate rendering work.

## Consequences

- A scene the player faces gets its files and its ticks before a scene at the same distance
  behind them; turning round reorders the tick and download queues within a frame.
- Strict priority can hold a far scene's downloads while nearer scenes keep requesting.
  The load distance bounds how many scenes compete. The
  avatar tier is strict as well: while any avatar's wearables wait, no scene but the player's
  gets a slot, so a crowd delays the neighbouring scenes' files by its wearables.
- A scene behind the camera ticks as before while frame time is spare, and is the first to tick
  less often when it is not; the player's scenes never yield.
- NFT images, profile pictures, Fontsource font families and imposter archives name no scene,
  and an avatar's emote or a backpack thumbnail names a hash no live entity holds, so they
  download after every live scene's and avatar's files, however near the NFT frame is. A
  texture or font a scene gives as a URL goes through its content-file path and takes its key;
  JavaScript `fetch` takes no request slot at all.
- On the headless server nothing is behind, so no scene's tick key is penalised; its scenes
  order by distance from the player only.

## Follow-ups

- Carry the scene on URL-form asset paths (NFT images, Fontsource fonts) so they take their
  scene's key instead of the flat rank after the live scenes.
- Emotes load under their own entity hashes like wearables; filing them under the avatar's
  partition is the same mechanism (list them in the avatar's `PartitionedDownloads`), not done
  here; until then an avatar's emote ranks in `Other`.
- Spawn the render avatar from the start of loading, so one entity owns an outfit from the
  first pointer lookup to despawn and `PartitionedDownloads` lives on it, cleaned up for free on
  an outfit change or a selection removed (robtfm's review suggestion). Not done here: the
  render child only spawns once every wearable has resolved, so spawning it earlier means a
  "still resolving" state through the unchanged-definition check, the `PreviousAvatar` swap and
  the processing systems. Until then the component stays on the avatar root, and
  `update_render_avatar` drops it when the selection goes.
- Loading waves sort the newly required scenes by their partition and take the first N;
  until then spawn order is hash-map order.
- `scene_timing` (`scene_runner::load_timing`) logs, per scene, the time from spawn to its fifth
  tick and, per arrival, the time until the player's scene and its first ring are ready. It is
  instrumentation, not part of the decision: run it on `main` (the commit that adds it applies
  there alone) and on this branch to see whether the order shortens the player's load.
