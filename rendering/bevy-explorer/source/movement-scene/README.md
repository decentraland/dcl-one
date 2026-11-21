# movement portable scene

Scene to implement standard movement for decentraland clients.

## Origin

Imported from https://github.com/robtfm/movement-scene at revision
`72751a896fa7a624eef14720085894f570561078`, with its author's agreement to publish it here under
the Explorer's Apache-2.0 license (`../LICENSE`). Editor and assistant configuration files, the asset
tooling with its source models and the change log from that repository were not imported. The
Explorer loads the deployed scene as the portable `basiccontroller.dcl.eth`.

## Build

The scene installs with pnpm, like the Explorer's other JavaScript projects
([decision](../../../../docs/adr/2026-11-13-pnpm-default-package-manager.md)):

```sh
corepack pnpm@12.21.0 install --frozen-lockfile
corepack pnpm@12.21.0 run build
```

`build` bundles `src/index.ts` into `bin/game.js` and type-checks it.

## Ground handling

The engine's collide-and-slide (step 6 of the algorithm described in
`library/protocol/proto/decentraland/sdk/components/avatar_movement.proto`) handles the ground for
an avatar that starts the step on it:

- a surface steeper than 46.5 degrees stops the horizontal move into it;
- the move follows walkable ground, up a slope at its horizontal speed and down a slope or a drop
  of up to 0.15m;
- the avatar steps onto a riser up to 0.45m high.

The scene leaves these to the engine, so it is deployed together with an Explorer that does them.
