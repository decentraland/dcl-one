# ADR: Keep the Explorer's movement scene in the Explorer tree

- Status: Accepted
- Date: 2026-11-16

## Context

The Explorer does not compute the local player's walking, running and jumping velocity itself. A
portable SDK7 scene does, and the engine applies the velocity it returns. That scene is deployed
as the world `basiccontroller.dcl.eth`. Its source lived in a separate personal repository,
https://github.com/robtfm/movement-scene, which has no license file.

Movement changes touch the scene and the engine together, and are judged by measurements of
both, such as the scene's deceleration and its dependence on the frame rate. With the source
outside this tree, each such change is reviewed and tested in two places, and the missing license
file leaves its terms unstated.

## Decision

Import the scene into `rendering/bevy-explorer/source/movement-scene/`, beside the HUD's bridge
scene (`source/react-web/bridge-scene`).

- **Licence.** Its author agreed to publish it here under the Explorer's Apache-2.0 license
  (`rendering/bevy-explorer/source/LICENSE`).
- **What is imported.** The scene's source, the assets it loads and its manifests, at upstream
  revision `72751a896fa7a624eef14720085894f570561078`. Editor and assistant configuration files are
  left out (`.editor/`, `cursor*`, `CLAUDE.md`, `dclcontext/`), and so are the asset tooling
  (`source/`) with the models it reads, and the change log (`CHANGES.md`).
- **Git LFS.** The scene's models, images and sounds (`*.glb`, `*.png`, `*.wav`) are stored in
  Git LFS, as the e2e scenes' are.
- **Tooling.** The scene keeps its own `package.json` and is built with the SDK scene tooling.
  It is not part of the Cargo workspace.
- **No pin file.** This is not a vendored component. The tree's copy is the source of truth, so
  there is no `upstream.lock.json` or patch series.

## Consequences

- Movement changes are reviewed with the engine changes and the measurements that motivate them.
- Deploying the world `basiccontroller.dcl.eth` from this directory is a separate step; clients
  load whatever that world has deployed. For local testing, serve the scene and pass
  `?portables=` on web or `--portables` on native.
- The upstream repository may keep changing. Anything wanted from it is ported here by hand.
