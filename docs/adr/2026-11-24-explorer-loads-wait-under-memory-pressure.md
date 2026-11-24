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

- **One latch.** `LoadRefusal` turns on at Full, holds through Warning and turns off at Normal,
  Abundance or when there is no reading. The 5 % gap keeps a reading at the threshold from
  admitting and refusing on alternate samples.
- **Scenes.** While refusing, the scene lifecycle spawns only the scene the player stands in,
  portables and system scenes. Spawned scenes are kept. An imposter bake's scene waits.
  `RefusedScenes` counts the scenes held back each frame.
- **Downloads.** While refusing, the request slots grant only tiers before `Avatar`: the
  player's own body, wearables and emotes, avatar previews the player opens (`PlayerDownloads`),
  the scenes the player stands in, portables, system scenes and the SDK6 adaption layer
  (`SCENE_RUNTIME_OWNER`). Other avatars, other scenes, stand-ins, URL requests and departed
  scenes keep their place in the queue. A held request is granted as soon as it rises out of
  the held tiers: its hash is registered for the player, the player's scene shares its
  download, or the player walks into its scene.
- **Idle metadata.** While refusing, collectible metadata that no caller has requested in the
  last three frames is released; a later request loads it again.
- **Visibility.** The Memory and limits debug section shows the latch, the scenes held back and
  the first held tier. Lowering the session memory limit there, or *Simulate Full pressure*,
  triggers the latch.

## Rejected

- **Holding every download, the player's included.** The player's scene and avatar could stay
  incomplete for the rest of the session, since the refusal frees nothing that is in use.
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
  lasts; both resume below the warning threshold, all held scenes spawning in the same frame.
- The refusal slows growth; it does not cap it. Unaffected: the player's own loads, reads served
  by the disk or web cache (they take no request slot), instantiation of data already
  downloaded, and entity resolution, which posts outside the request slots.
- Natively the reading excludes the scene runtime process and other applications, so refusal
  starts late on a machine whose memory other programs use; allocator retention can keep the
  reading in Warning after usage drops. GPU memory is not part of the reading.
- While a held request waits, download priorities are published every frame, so a neighbour
  the player walks into is released at once.
