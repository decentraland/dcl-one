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

- **One latch.** `LoadRefusal` turns on at Full. It holds through Warning for at most
  `WARNING_HOLD` (30 s) after the last Full reading, and turns off at Normal, Abundance or when
  there is no reading. The gap between the thresholds keeps a reading at the full threshold from
  admitting and refusing on alternate samples; the time limit keeps memory that stays resident
  after it is freed from holding loads for the rest of the session.
- **Scenes.** While refusing, the scene lifecycle spawns only the scene the player stands in,
  portables and system scenes. Spawned scenes are kept. An imposter bake's scene waits, and the
  bake's fallback countdown restarts while it does. `RefusedScenes` counts the scenes held back
  each frame.
- **Downloads.** While refusing, the request slots grant only tiers before `Avatar`: the
  player's own body, wearables and emotes, avatars that system scenes place (the backpack, a
  passport: `PlayerDownloads`), the scenes the player stands in, portables, system scenes, and
  URL loads tagged as the player's (the SDK6 adaption layer, the NFT dialog's image). A URL load
  a scene makes for an NFT frame ranks under that scene's hash. Other avatars, other scenes,
  stand-ins, other URL requests and departed scenes keep their place in the queue. A held
  request is granted as soon as it rises out of the held tiers: its hash is registered for the
  player, the player's scene shares its download, or the player walks into its scene. While the
  hold is on, download priorities are published only on frames in which a partition, a holder or
  the player's scenes change.
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
  several scenes load by URL (a font) would load once per scene. Only NFT frames, which are
  rarely shared, are tagged with their scene.
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
- Fonts that scenes load by URL wait, the player's scene's included, so its text uses the
  fallback font until the refusal ends.
- The refusal slows growth; it does not cap it. Unaffected: the player's own loads, reads served
  by the disk or web cache (they take no request slot), instantiation of data already
  downloaded, and entity resolution, which posts outside the request slots.
- Natively the reading excludes the scene runtime process and other applications, so refusal
  starts late on a machine whose memory other programs use. After `WARNING_HOLD` in Warning,
  loads resume even if usage has not dropped; if it reaches Full again, they wait again. GPU
  memory is not part of the reading.
