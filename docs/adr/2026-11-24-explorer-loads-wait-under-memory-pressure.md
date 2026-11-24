# Explorer loads wait while engine memory is full

- Status: Accepted
- Date: 2026-11-24

## Context

`MemoryStatus` classifies the engine's memory reading as Abundance, Normal, Warning or Full
(below 65 %, 65–70 %, 70–75 %, above 75 % of the cap). Natively the reading is the engine
process's resident memory against physical RAM or the cgroup limit; on the web it is the live
allocations of the engine heap against its 4 GiB ceiling. Without a reaction, every scene that
comes into range spawns and every download is granted however full memory is, so a walk through
a dense area grows the engine until the platform kills it.

## Decision

- **One latch.** `EngineMemoryPressureMode` becomes `RefusalDueFull { full_at }` at Full. It
  holds through Warning for at most `WARNING_HOLD` (30 s) after `full_at`, the last Full reading,
  and returns to `Normal` at Normal, Abundance or when there is no reading. The gap between the thresholds keeps a reading at the full threshold from
  admitting and refusing on alternate samples; the time limit keeps memory that stays resident
  after it is freed from holding loads for the rest of the session.
- **Scenes.** While refusing, the scene lifecycle spawns only the scene the player stands in,
  portables and system scenes. Spawned scenes are kept. An imposter bake's scene waits, and the
  bake's fallback countdown restarts while it does. `RefusedScenes` counts the scenes held back
  each frame.
- **Downloads.** While refusing, the request slots grant only tiers before `Avatar`: the
  player's own body, wearables and emotes, avatars that system scenes place (the backpack, a
  passport: `PlayerDownloads`), the scenes the player stands in, portables, system scenes, and
  URL loads tagged as the player's: small files every scene shares (the SDK6 adaption layer,
  fonts) and profile pictures in UI the player opens. An NFT image or a profile picture a
  scene shows ranks under that scene's hash, and the NFT dialog a scene opens shares its
  frame's image. Other avatars, other scenes, stand-ins,
  other URL requests and departed scenes keep their place in the queue. A held request is
  granted as soon as it rises out of the held tiers: its hash is registered for the player, the
  player's scene shares its download, or the player walks into its scene. While the hold is on,
  download priorities are published only when a scene's place among the player's scenes
  differs from the key the gate holds for it; other keys only order held requests.
- **Headless.** The scene server runs no refusal: its reading spans every tenant scene, and
  holding their downloads would leave its colliders behind its clients'.
- **Idle metadata.** While refusing, collectible metadata that no caller has requested in the
  last three frames is released; a later request loads it again. Waiting loads stop growth, but
  only released memory can bring the reading back under the warning threshold.
- **Visibility.** The Memory and limits debug section shows the latch, the scenes held back and
  the first held tier. Lowering the session memory limit there, or *Simulate Full pressure*,
  triggers the latch.

## Rejected

- **Holding every download, the player's included.** The player's scene and avatar could stay
  incomplete for as long as memory stays full.
- **Giving every URL load its scene as owner.** The owner is part of the asset path, so a file
  several scenes load by URL (a font) would load once per scene. Shared files take the one
  player owner instead; only NFT images, which are rarely shared, are tagged with their scene.
- **Unloading scenes or shrinking the live radius at Warning.** It needs a lower representation
  to step down to; it is a separate decision.
- **Per-scene memory estimates.** Fixed per-parcel figures are guesses; the measured reading is
  what is available.
- **`LiveScenes::block_new_scenes`.** It stops every spawn, including the player's own scene and
  system scenes.
- **Releasing emote and wearable representations under pressure.** Animation graphs and avatar
  definitions use them without owning them; releasing them needs those owners first.

## Consequences

- Neighbouring scenes stall half-loaded and other avatars keep what they had while the refusal
  lasts; both resume when it ends, the held scenes spawning in the usual loading waves.
- The owner is part of the asset path, so a profile picture shown both in the player's UI and
  by scenes is decoded once per owner. The pictures are small; one shared path would let a
  neighbour's requests ride on the player's rank.
- The player's URL loads rank with the player's scenes at all times, ahead of other avatars and
  neighbouring scenes even when memory is fine; they are few and small.
- When the refusal ends, the free slots go out at once on the keys of the last publish, which
  under the hold tracked only the player's scenes; later grants use fresh keys.
- The refusal slows growth; it does not cap it. Unaffected: the player's own loads, reads served
  by the disk or web cache (they take no request slot), instantiation of data already
  downloaded, and entity resolution, which posts outside the request slots.
- Natively the reading excludes the scene runtime process and other applications, so refusal
  starts late on a machine whose memory other programs use. After `WARNING_HOLD` in Warning,
  loads resume even if usage has not dropped; if it reaches Full again, they wait again. GPU
  memory is not part of the reading.
