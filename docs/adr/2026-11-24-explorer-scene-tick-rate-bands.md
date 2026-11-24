# ADR: The Explorer caps each scene's tick rate by distance band

- Status: Accepted
- Date: 2026-11-24
- Supersedes: [nearest-first priority](2026-11-02-explorer-nearest-first-priority.md), for its
  rejection of a tick-rate table and of a cap behind the camera; its partition, tick key and
  download ordering remain.

## Context

The scene loop sends the next tick to the eligible scene with the lowest key, squared distance
over time since last sent, four times farther behind the camera, under the scene thread limit.
A far scene still ticks whenever it wins a slot, so every scene in load range costs scene-thread
and main-thread time each frame there is spare time, whether or not the player can see it change.

## Decision

- **A maximum rate per distance band.** `scene_tick_rate` reads a scene's `Partition`: under
  16 m from the player to its nearest parcel, 40 ticks a second; under 32 m, 30; under 48 m, 15;
  under 64 m, 10; under 80 m, 5; from 80 m on, none (`SCENE_TICK_RATE_BANDS`). Distance is the
  partition's, from the player to the nearest point of the scene's nearest parcel.
- **Behind the camera, at most once a second** (`BEHIND_SCENE_TICK_RATE`), in a band that ticks.
- **A rate is a minimum interval.** `update_scene_priority` leaves a capped scene out of the
  frame's queue until `1 / rate` seconds have passed since it was last sent. A scene that is due
  is ordered by the existing key, so the cap only removes ticks; the frame rate quantizes the
  interval to whole frames.
- **Full rate.** The player's scenes, the scenes at the player's position while out of world,
  portable and global scenes, the named priority scenes, the system scene, the scene being baked
  into an imposter, and every scene before `SCENE_LOADED_TICK` (so loading, which waits on a
  scene's first ticks, never stalls on a frozen band).
- **Only where a camera renders.** Without a `PrimaryCamera` that has a `Camera`, as on the
  headless authoritative server, no scene is capped.
- **Inspectable.** Each scene's context holds its current cap (`tick_rate_cap`), and
  `SceneUpdates::skipped_rate_capped` counts the scene-frames the cap skipped; the runtime debug
  panel's "Scene Tick Rates" section (`/scene_tick_rate_state`) shows both.

## Consequences

- Scenes from 80 m on keep their last state until the player comes nearer; their SDK systems,
  tweens driven from script and network messages wait until then.
- A scene behind the camera delivers up to a second of changes in one reply. Whether that
  hitches when the camera turns is open until main-thread and scene-thread time are measured at
  a dense spot with and without the cap.
- Bands and rates are code constants, not settings.

## Rejected

- **Keeping only the aging key.** It shares spare frame time by distance but never removes far
  ticks, so far scenes still take scene threads and main-thread receive and apply time.
- **Capping the player's scene or portables.** They are what the player interacts with, and
  their tick rate is the input latency the player feels.
