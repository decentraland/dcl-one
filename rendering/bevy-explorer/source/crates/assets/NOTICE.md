# Third-party assets

The files in `src/assets/lobby_stage/` are made from art in
[decentraland/unity-explorer](https://github.com/decentraland/unity-explorer), under the Apache
License 2.0 (`LICENSE` at the root of this workspace); the licence applies to these files.
Copyright Decentraland.

`tooling/lobby-stage-assets.py` converts them: each model becomes one GLB holding its own
textures, and the pictures are re-encoded (JPEG) or resized. They are otherwise unchanged.

| Files | Source in unity-explorer, under `Explorer/Assets/` |
|---|---|
| `candle03.glb`, `candle04.glb`, `pumpkin1.glb` … `pumpkin4.glb` | the Halloween props and their textures (`DCL/LobbyStage/Props/Halloween`) |
| `rock01.glb` … `rock03.glb` | the stylized rocks (`DCL/Landscape/Assets/Rocks`) and their textures (`DCL/Landscape/Assets/Textures`); the smoothness, metallic and emission values come from the rock materials in `DCL/Landscape/Assets/GPUI/Rocks` |
| `podium.glb` | the character preview platform (`DCL/Character/CharacterPreview/Assets/PreviewPlatform`) |
| `halloween_backdrop.jpg`, `game_world_backdrop.jpg`, `ground.jpg` | the lobby's backgrounds and ground (`DCL/Lobby/Textures`) |
| `blob_shadow.png` | the radial glow (`Textures/Common/RadialGlow.png`), baked to one grey channel |

This file sits outside `src/assets` because every file there is embedded in the engine.
