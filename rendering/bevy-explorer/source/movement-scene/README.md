# movement portable scene

Scene to implement standard movement for decentraland clients.

## Origin

From https://github.com/robtfm/movement-scene at `72751a896fa7a624eef14720085894f570561078`
([decision](../../../../docs/adr/2026-11-16-explorer-movement-scene-in-tree.md)).

## Build

The scene installs with pnpm, like the Explorer's other JavaScript projects
([decision](../../../../docs/adr/2026-11-13-pnpm-default-package-manager.md)):

```sh
corepack pnpm@12.21.0 install --frozen-lockfile
corepack pnpm@12.21.0 run build
```

`build` bundles `src/index.ts` into `bin/game.js` and type-checks it.

## Ground handling

The engine moves the avatar by collide-and-slide alone (step 6 of the algorithm described in
`library/protocol/proto/decentraland/sdk/components/avatar_movement.proto`), so the scene keeps it
on the ground in its reply (`src/vertical.ts`, `src/horizontal.ts`):

- slope limit: on the ground, the move into a slope steeper than 46.5 degrees is cut to reach it
  no sooner than the next reply lands, where collide-and-slide would slide the avatar up it;
- ground following: on the ground, the reply adds the vertical speed that moves it along the
  walkable ground under its centre, so a move up a slope keeps its horizontal speed and a move
  down one keeps to it;
- ground snap: off ground it stood on, the avatar follows walkable ground down by up to 0.15m
  beyond the ground's slope, measured from the ground or the edge it stood on; a larger drop is an
  edge it walks off;
- step up: the avatar climbs a riser from 0.05m to 0.45m high. A move at 5.5m/s or faster climbs
  ahead of the riser and reaches the tread with the face; a slower one climbs once it touches the
  face, over 0.1s.

It reads the ground with four casts a tick: the avatar's capsule and a ray down from the feet,
the capsule along the facing (1.3m) and a ray down onto the tread past the face that cast finds.
