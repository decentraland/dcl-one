# Engine lobby stage

- Status: Accepted
- Date: 2026-11-13

## Decision

The Explorer engine draws the lobby's 3D stage. The system (bridge) scene keeps the
preview avatar, its camera and its turns and emotes, and names the stage it wants
through one system API call, `setLobbyStage`: a look's id, the scene camera layer
the avatar and camera render on, and the avatar's feet in the scene's coordinates.
`null` takes the stage down.

The stage module (`crates/system_ui/src/lobby_stage`) builds the set under the
system scene's root on the render layer of that camera, so it moves with the scene
and only that camera sees it. Every frame it refits the backdrop to the camera:
the floor dissolves along a band ending at the look's screen height, and the
backdrop covers the frame above it. The set's surfaces are engine materials (floor,
mist, dust, contact shadow, vignette, mossy rocks) and embedded GLB props. The
looks are engine data, listed for the HUD by id in `library/explorer-core/engine/lobbyStage.ts`;
the HUD stores the chosen id (Settings → Lobby) and sends it with the lobby region.

## Consequences

Scenes, including the system scene, cannot use custom shaders, and the stage's
dissolving floor, drifting mist, dust and screen-space vignette need them, so the
engine owns them rather than the scene. The scene's other pages that borrow the
lobby region (the guest start and welcome pages) show the same stage.

A new look is engine data plus embedded assets, and an id in `lobbyStage.ts`; the
models come from `tooling/lobby-stage-assets.py`. Unknown ids show the first look,
so a HUD ahead of or behind the engine still gets a stage.

Revisit this choice if scenes gain custom materials, or if the stage must show
outside the system scene's camera.
