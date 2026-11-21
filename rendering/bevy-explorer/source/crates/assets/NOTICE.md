# Third-party assets

The files in `src/assets/lobby_stage/` and `src/assets/sky/` are made from art in
[decentraland/unity-explorer](https://github.com/decentraland/unity-explorer), under the Apache
License 2.0 (`LICENSE` at the root of this workspace); the licence applies to these files.
Copyright Decentraland.

`tooling/lobby-stage-assets.py` converts the lobby stage: each model becomes one GLB holding its
own textures, and the pictures are re-encoded (JPEG) or resized. They are otherwise unchanged.

| Files | Source in unity-explorer, under `Explorer/Assets/` |
|---|---|
| `candle03.glb`, `candle04.glb`, `pumpkin1.glb` … `pumpkin4.glb` | the Halloween props and their textures (`DCL/LobbyStage/Props/Halloween`) |
| `rock01.glb` … `rock03.glb` | the stylized rocks (`DCL/Landscape/Assets/Rocks`) and their textures (`DCL/Landscape/Assets/Textures`); the smoothness, metallic and emission values come from the rock materials in `DCL/Landscape/Assets/GPUI/Rocks` |
| `podium.glb` | the character preview platform (`DCL/Character/CharacterPreview/Assets/PreviewPlatform`) |
| `halloween_backdrop.jpg`, `game_world_backdrop.jpg`, `ground.jpg` | the lobby's backgrounds and ground (`DCL/Lobby/Textures`) |
| `blob_shadow.png` | the radial glow (`Textures/Common/RadialGlow.png`), baked to one grey channel |
| `sky/horizon_noise.png` | the stylized skybox's horizon noise (`DCL/StylizedSkybox/Textures/Noise4.png`), halved to 1024 a side, the size Unity imports it at, and stored as its one grey channel |
| `sky/clouds_overhead.png`, `sky/clouds_low.png`, `sky/clouds_cumulus.png` | the stylized skybox's cloud strips (`DCL/StylizedSkybox/Textures/Clouds/SkyboxCloudsOverhead_V2.png`, `SkyboxCloudsLow_V2.png`, `SkyboxCloudsCumulus_V2.png`), unchanged |

These files in `src/assets/sounds/ui/` are unchanged copies of sounds in unity-explorer, under the
same licence. Copyright Decentraland.

| Files | Source in unity-explorer, under `Explorer/Assets/DCL/Audio/Sounds/Explorer Alpha 2024/` |
|---|---|
| `generic_button_hover.wav`, `generic_button_press.wav` | `Generic - Button - Highlight.wav`, `Generic - Button - Press.wav` |
| `mainmenu_widget_open.wav` | `Main Menu - Widget - Open.wav` |
| `toggle_enable.wav`, `toggle_disable.wav` | `Toggle - Enable.wav`, `Toggle - Disable.wav` |
| `voice_chat_call_tune.mp3`, `voice_chat_join.wav`, `voice_chat_leave.wav` | `Chat - Voice - Call Tune.mp3`, `Chat - Voice - Join.wav`, `Chat - Voice - Leave.wav` |
| `voice_chat_mic_on.wav`, `voice_chat_mic_off.wav` | `Chat - Voice - Microphone On.wav`, `Chat - Voice - Microphone Off.wav` |
| `voice_chat_raise_hand.wav` | `Jingle - Add Favorite.wav` |
| `widget_chat_message_private_send.wav` | `Widget - Chat - Message - Private - Send.wav` |
| `widget_emotes_open.wav`, `widget_emotes_close.wav`, `widget_emotes_highlight.wav` | `Widget - Emotes - Open.wav`, `Widget - Emotes - Close.wav`, `Widget - Emotes - Button Highlight.wav` |

This file sits outside `src/assets` because every file there is embedded in the engine.
