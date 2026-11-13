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

## Consequences

- A live scene no longer respawns at the load edge, and "Scene Unload Distance" does what its
  description says.
- Entering a dense area with a large load distance spawns six scenes, nearest ring and in front
  first, then one more as each comes up, instead of every scene in range in one frame.
- At the default 10 m the live ring decides: up to 9 scenes among one-parcel scenes, 25 inside
  the keep ring. Waves rarely queue a scene there; they matter once the load distance is raised.
- Whether 1.5 s is enough depends on the time a scene takes from spawn to shown, which has not
  been measured.
- A scene stalled in loading holds a wave slot for 20 s, then the wave moves on with it still
  loading.
- Loading scenes keep their animated walls; what shows while a scene loads is a later decision.
