> A curated, AI-readable index of Decentraland SDK7 example scenes, mechanics, and reusable systems.

# Example Scenes

Explore these projects to learn how other creators implement scene mechanics. Every entry links to its source repository for further study.

This is an index, not a second copy of the code. Examples stay in the repositories that maintain them.


> [!NOTE]
> **About these examples:** This catalog includes Decentraland-maintained and community-maintained projects. Community projects are maintained independently and may change over time. Inclusion does not imply a security audit or guarantee of safety. Review code, dependencies, and installation scripts before running or reusing an example. Maintenance labels reflect the source repository's GitHub owner, not a security assessment.


AI Agent? Go here: [catalog.json](https://raw.githubusercontent.com/decentraland/docs/main/creator/tutorials/examples/catalog.json). It contains the same entries and metadata in a machine-readable format.

<!--
	EDITORS READ THIS: Don't edit below this line. Instead, edit the files in the ./examples/ directory. From the repository root, install dependencies with npm ci --ignore-scripts and run npm run generate:examples to update the tables below.
-->
<!-- example-scenes-catalog:start -->
## Start here

Core SDK7 references and starting points for new scenes.

| Example | What it demonstrates | Maintained by | Tags | Source |
| --- | --- | --- | --- | --- |
| [SDK7 scene template](/docs/creator/tutorials/examples/getting-started/sdk7-scene-template/) | A clean starting point for a new TypeScript SDK7 scene. | Decentraland | `sdk7`, `scene` | [Open repository](https://github.com/decentraland/sdk7-scene-template) |

## Published Games

Released Decentraland scenes that demonstrate real-world mechanics.


> [!WARNING]
> **Community-maintained code — review before use**
>
> Community projects listed here are maintained outside the Decentraland Foundation and DAO GitHub organizations. Their source code and live deployments may change independently.
>
> Inclusion in this catalog does not imply a security audit or guarantee of safety. Review code, dependencies, and installation scripts before running or reusing it.
>
> [Report a catalog concern](https://github.com/decentraland/docs/issues/new). For suspected vulnerabilities, follow the upstream project's security policy. Do not include exploit details or credentials in public issues.


| Game                                                                   | What it demonstrates                                                                                                                      | Maintained by               | Tags                                                                                                                        | Source                                                                          |
| ---------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------- | --------------------------- | --------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------- |
| [Alien Scrapyard](/docs/creator/tutorials/examples/published-games/alien-scrapyard/)       | Cooperative building rounds with data-defined construction puzzles and phase-driven cinematic cameras.                                    | Community · procesista      | `game`, `multiplayer`, `timed-rounds`, `camera`, `cinematic`                                                                | [Open repository](https://github.com/procesista/Alienscrap)                     |
| [Astro Crew](/docs/creator/tutorials/examples/published-games/astro-crew/)                 | Cooperative spaceship combat with server-owned game state, persistent progression, turret cameras, and screen-edge objective markers.     | Community · DansStuff       | `game`, `multiplayer`, `multiplayer-server`, `progression`, `leaderboard`, `storage`, `camera`, `ui`, `combat`              | [Open repository](https://github.com/DansStuff/AstroCrew)                       |
| [Bloom Garden](/docs/creator/tutorials/examples/published-games/bloom-garden/)             | A cooperative garden with persistent plant state, server-clock offset estimation, and pooled petal animations.                            | Community · TheLivingGarden | `game`, `multiplayer`, `multiplayer-server`, `storage`, `leaderboard`, `animation`, `effects`                               | [Open scene](https://github.com/TheLivingGarden/TheLivingGarden/tree/main-main) |
| [Clean The Club](/docs/creator/tutorials/examples/published-games/clean-the-club/)         | Cooperative nightclub cleanup with shared component polling and avatar-attached props that reflect carried items.                         | Community · CleanTheClub    | `game`, `multiplayer`, `timed-rounds`, `avatars`, `performance`, `interaction`                                              | [Open repository](https://github.com/CleanTheClub/CleanTheClub)                 |
| [Cozy Farm](/docs/creator/tutorials/examples/published-games/cozy-farm/)                   | A social farming game with timed crop-growth stages, fertilizer modifiers, and repeated farms built from one entity hierarchy.            | Decentraland DAO            | `game`, `social`, `progression`, `assets`, `scene`                                                                          | [Open repository](https://github.com/dcl-regenesislabs/cozy-farm)               |
| [Dead Surge](/docs/creator/tutorials/examples/published-games/dead-surge/)                 | Zombie survival with weapon progression, pooled reward text, and local enemy steering around other zombies and obstacles.                 | Decentraland DAO            | `game`, `combat`, `progression`, `movement`, `ui`, `performance`                                                            | [Open repository](https://github.com/dcl-regenesislabs/dead-surge)              |
| [Fastlane Bowling](/docs/creator/tutorials/examples/published-games/fastlane-bowling/)     | Multiplayer bowling with server-simulated physics, persistent progression, batched state updates, and mobile-scaled particle effects.     | Community · stom66          | `game`, `studio-made`, `multiplayer`, `multiplayer-server`, `physics`, `progression`, `leaderboard`, `storage`, `particles` | [Open repository](https://github.com/stom66/dcl-bowling)                        |
| [Flag Tag](/docs/creator/tutorials/examples/published-games/flag-tag/)                     | Multiplayer keep-away with timed rounds, persistent leaderboards, pooled projectiles, and a proximity-based lighting budget.              | Community · iillee          | `game`, `studio-made`, `multiplayer`, `multiplayer-server`, `leaderboard`, `timed-rounds`, `lighting`, `performance`        | [Open repository](https://github.com/iillee/flagtag)                            |
| [Goal Legends](/docs/creator/tutorials/examples/published-games/goal-legends/)             | Multiplayer penalty shootouts with server-run matches, persistent leaderboards, phase-driven cameras, and synchronized ball animations.   | Community · carlosmu        | `game`, `multiplayer`, `multiplayer-server`, `storage`, `leaderboard`, `camera`, `animation`                                | [Open repository](https://github.com/carlosmu/goal-legends-arena)               |
| [Kick Off](/docs/creator/tutorials/examples/published-games/kick-off/)                     | A World Cup prediction scene with charged ball kicks, immediate local motion feedback, and responsive schedule panels.                    | Decentraland DAO            | `game`, `seasonal`, `input`, `multiplayer`, `ui`                                                                            | [Open repository](https://github.com/dcl-regenesislabs/kickoff-2026)            |
| [Monster Recon](/docs/creator/tutorials/examples/published-games/monster-recon/)           | Memory matching followed by trigger-based monster chases, with collectible progression, leaderboards, and custom bitmap text.             | Community · carlosmu        | `game`, `collection`, `leaderboard`, `triggers`, `ui`, `sprites`, `animation`                                               | [Open repository](https://github.com/carlosmu/dcl-monster-recon)                |
| [My Dear Pet](/docs/creator/tutorials/examples/published-games/my-dear-pet/)               | Pet collecting with acceleration and braking for movement, plus phase-offset animation of editor-placed eggs.                             | Decentraland DAO            | `game`, `collection`, `movement`, `animation`                                                                               | [Open repository](https://github.com/dcl-regenesislabs/marsh-colony)            |
| [Pigeon Deluxe](/docs/creator/tutorials/examples/published-games/pigeon-deluxe/)           | Timed parcel delivery with pickup and drop-off areas, an avatar-mounted direction pointer, and scoring separated from delivery animation. | Community · PepeGawd        | `game`, `community-made`, `timed-rounds`, `triggers`, `avatars`, `score`                                                    | [Open repository](https://github.com/PepeGawd/Pigeon-Deluxe)                    |
| [Prime Drive](/docs/creator/tutorials/examples/published-games/prime-drive/)               | Three-lane racing with recycled scenery, distance-based record ghosts, and persistent leaderboard data.                                   | Community · carlosmu        | `game`, `multiplayer-server`, `leaderboard`, `storage`, `movement`, `performance`                                           | [Open repository](https://github.com/carlosmu/dcl-prime-drive)                  |
| [Shroom Zoom](/docs/creator/tutorials/examples/published-games/shroom-zoom/)               | Platforming with trigger-activated bounce raycasts, player impulses, and collectible sprites that follow or orbit their owner.            | Community · CanessaDCL      | `game`, `platformer`, `collectibles`, `triggers`, `raycast`, `physics`, `animation`, `sprites`                              | [Open repository](https://github.com/CanessaDCL/dcl-shroomzoom)                 |
| [Sky Chaser](/docs/creator/tutorials/examples/published-games/sky-chaser/)                 | Multiplayer package-catching rounds with persistent leaderboards, pooled projectiles, and pickup placement seeded from round state.       | Community · stom66          | `game`, `studio-made`, `multiplayer`, `multiplayer-server`, `leaderboard`, `performance`, `timed-rounds`                    | [Open repository](https://github.com/stom66/dcl-sky-chaser)                     |
| [Slay the Steps](/docs/creator/tutorials/examples/published-games/slay-the-steps/)         | A multiplayer Festive Trail scene with custom avatars, Marketplace integration, a raycast-driven color wheel, and tracking cameras.       | Community · stom66          | `game`, `studio-made`, `multiplayer`, `multiplayer-server`, `avatars`, `marketplace-api`, `ui`, `camera`, `raycast`         | [Open repository](https://github.com/stom66/dcl-slay-the-steps)                 |
| [Slide Penguin](/docs/creator/tutorials/examples/published-games/slide-penguin/)           | Penguin sliding with timed sweeping that modifies deceleration and scoring based on distance from the target.                             | Community · ToxSam          | `game`, `seasonal`, `platformer`, `input`, `movement`, `score`                                                              | [Open repository](https://github.com/ToxSam/SlidePenguin)                       |
| [The Fortune Teller](/docs/creator/tutorials/examples/published-games/the-fortune-teller/) | Social fortune-telling with explicit conversation-state transitions and a pivot-based cinematic camera rig.                               | Community · carlosmu        | `game`, `social`, `multiplayer`, `dialogue`, `camera`, `cinematic`                                                          | [Open repository](https://github.com/carlosmu/circus_fortune_teller)            |
| [Tower of Madness](/docs/creator/tutorials/examples/published-games/tower-of-madness/)     | Timed platforming through changing tower sections, with asset preloading and a skippable camera tour of the level.                        | Decentraland DAO            | `game`, `platformer`, `timed-rounds`, `loading`, `assets`, `camera`, `cinematic`                                            | [Open repository](https://github.com/dcl-regenesislabs/towerofmadness)          |

## Gameplay and interaction

Core gameplay mechanics and player interactions.

| Example | What it demonstrates | Maintained by | Tags | Source |
| --- | --- | --- | --- | --- |
| [Input modifier](/docs/creator/tutorials/examples/gameplay-interaction/input-modifier/) | Changing or constraining player movement input in part of a scene. | Decentraland | `movement`, `input`, `player` | [Open scene](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/0,1-input-modifier) |
| [Particle system](/docs/creator/tutorials/examples/gameplay-interaction/particle-system/) | Fire, smoke, sparks, snow, and other particle effects. | Decentraland | `particles`, `effects`, `visuals` | [Open scene](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/0,7-particle-system) |
| [Pointer lock control](/docs/creator/tutorials/examples/gameplay-interaction/pointer-lock-control/) | Capturing mouse input for custom controls. | Decentraland | `input`, `mouse`, `camera` | [Open scene](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/31,20-pointer-lock-control) |
| [Raycasting and collider layers](/docs/creator/tutorials/examples/gameplay-interaction/raycasting-and-collider-layers/) | Pointing at scene geometry and controlling which colliders react to a raycast. | Decentraland | `interaction`, `raycast`, `collision` | [Open scene](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/77,-1-raycast-unit-tests) |
| [Trigger areas](/docs/creator/tutorials/examples/gameplay-interaction/trigger-areas/) | Detecting players entering and leaving areas, including collider-based triggers. | Decentraland | `interaction`, `triggers`, `collision` | [Open scene](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/75,-9-trigger-areas) |
| [Zombie Attack](/docs/creator/tutorials/examples/gameplay-interaction/zombie-attack/) | A complete, playable game scene with enemies and scoring. | Decentraland | `game`, `complete-scene`, `score` | [Open scene](https://github.com/decentraland/sdk7-goerli-plaza/tree/main/zombie-attack) |

## UI, camera, and player experience

Interface, camera, and player-facing systems.

| Example | What it demonstrates | Maintained by | Tags | Source |
| --- | --- | --- | --- | --- |
| [NPC Toolkit](/docs/creator/tutorials/examples/ui-camera-player/npc-toolkit/) | Reusable NPC behaviours and dialogue UI for SDK7 scenes. | Decentraland | `npc`, `dialogue`, `ui` | [Open repository](https://github.com/decentraland-scenes/dcl-npc-toolkit) |
| [Spectate mode](/docs/creator/tutorials/examples/ui-camera-player/spectate-mode/) | A spectator-style camera and player viewing flow. | Decentraland | `camera`, `spectator`, `player` | [Open scene](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/33,20-spectate-mode) |
| [UI animations](/docs/creator/tutorials/examples/ui-camera-player/ui-animations/) | Animated UI elements, sprite sheets, and loading indicators. | Decentraland | `ui`, `animation`, `sprites` | [Open scene](https://github.com/decentraland/sdk7-goerli-plaza/tree/main/ui-animations) |
| [UI safe areas](/docs/creator/tutorials/examples/ui-camera-player/ui-safe-areas/) | Positioning interface elements around screen insets and safe areas. | Decentraland | `ui`, `hud` | [Open scene](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/81,-2-ui-screen-inset-area) |
| [Virtual cameras](/docs/creator/tutorials/examples/ui-camera-player/virtual-cameras/) | Creating and switching to scene-defined cameras. | Decentraland | `camera`, `cinematic`, `controls` | [Open scene](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/2,22-virtual-cameras) |

## Multiplayer

Multiplayer patterns and server-owned shared state.

| Example | What it demonstrates | Maintained by | Tags | Source |
| --- | --- | --- | --- | --- |
| [Mini Games library](/docs/creator/tutorials/examples/multiplayer-data-services/mini-games-library/) | Player queues, scoreboards, and supporting logic for multiplayer mini games. | Decentraland | `multiplayer`, `scoreboard`, `game` | [Open repository](https://github.com/decentraland/sdk-mini-games) |
| [Multiplayer Server leaderboard](/docs/creator/tutorials/examples/multiplayer-data-services/authoritative-server-leaderboard/) | A server-owned leaderboard that validates scores, stores totals, and broadcasts rankings. | Decentraland | `multiplayer`, `leaderboard`, `server`, `storage` | [Open scene](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/90,-9-authoritative-server-leaderboard) |

## External services

Connecting scenes to external APIs, data sources, and third-party servers.

| Example | What it demonstrates | Maintained by | Tags | Source |
| --- | --- | --- | --- | --- |
| [Data from a spreadsheet](/docs/creator/tutorials/examples/external-services/spreadsheet-data/) | Scene content driven by a Google Sheet, updated without redeploying. | Decentraland | `api`, `spreadsheet`, `video`, `wearables` | [Open repository](https://github.com/decentraland/Read_Spreadsheet_Scene) |
| [Third-party guest book](/docs/creator/tutorials/examples/external-services/third-party-guest-book/) | A guest book backed by an external HTTP service. | Decentraland | `api`, `storage` | [Open scene](https://github.com/decentraland/sdk7-goerli-plaza/tree/main/guest-book-api) |
| [Third-party leaderboard](/docs/creator/tutorials/examples/external-services/third-party-leaderboard/) | A persistent leaderboard backed by an external HTTP service. | Decentraland | `api`, `storage`, `leaderboard` | [Open scene](https://github.com/decentraland/sdk7-goerli-plaza/tree/main/leader-board) |
| [Third-party player authentication](/docs/creator/tutorials/examples/external-services/third-party-player-authentication/) | Verifying player identity for requests sent to an external service. | Decentraland | `api`, `authentication`, `player` | [Open scene](https://github.com/decentraland/sdk7-goerli-plaza/tree/main/validate-player-authenticity) |

## Audio, video, and environment

Media, lighting, and environment controls.

| Example | What it demonstrates | Maintained by | Tags | Source |
| --- | --- | --- | --- | --- |
| [Audio visualization](/docs/creator/tutorials/examples/audio-video-environment/audio-visualization/) | Using live audio data to drive visual scene behaviour. | Decentraland | `audio`, `visualization`, `effects` | [Open scene](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/88,-10-audio-visualization) |
| [Dynamic lights](/docs/creator/tutorials/examples/audio-video-environment/dynamic-lights/) | Changing lights at runtime. | Decentraland | `lighting`, `environment`, `effects` | [Open scene](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/0,4-dynamic-lights) |
| [Skybox control](/docs/creator/tutorials/examples/audio-video-environment/skybox-control/) | Updating skybox time and transition settings from scene code. | Decentraland | `skybox`, `environment`, `lighting` | [Open scene](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/2,1-skybox-sdk-scene-a) |
| [Video streaming](/docs/creator/tutorials/examples/audio-video-environment/video-streaming/) | Playing streamed video on a scene surface. | Decentraland | `video`, `streaming`, `media` | [Open scene](https://github.com/decentraland/sdk7-goerli-plaza/tree/main/video-streaming) |

## Performance and scene systems

Asset loading and scene-level systems.

| Example | What it demonstrates | Maintained by | Tags | Source |
| --- | --- | --- | --- | --- |
| [Asset loading](/docs/creator/tutorials/examples/performance-scene-systems/asset-loading/) | Loading scene assets on demand rather than all at once. | Decentraland | `performance`, `loading`, `assets` | [Open scene](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/88,-12-asset-load) |
| [Material and texture gallery](/docs/creator/tutorials/examples/performance-scene-systems/material-and-texture-gallery/) | Material, texture, and GLTF node modifier patterns. | Decentraland | `materials`, `textures`, `rendering` | [Open scene](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/52,-52-testing-gallery) |
| [Modifier areas](/docs/creator/tutorials/examples/performance-scene-systems/modifier-areas/) | Applying area-specific settings such as hiding player name tags. | Decentraland | `player`, `areas`, `modifiers` | [Open scene](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/9,99-modifier-areas) |

## Scene management and feedback

Tools for managing scenes and collecting player feedback.

| Example | What it demonstrates | Maintained by | Tags | Source |
| --- | --- | --- | --- | --- |
| [Scene Feedback](/docs/creator/tutorials/examples/scene-management-feedback/playtest-feedback/) | Collect player ratings and comments during play or through a feedback button, kiosk, or area, with CSV results in scene Storage. | Decentraland | `ui`, `multiplayer-server`, `storage` | [Open scene](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/0,-1-scene-feedback) |

## Rewards

Scenes that give players wearables through Decentraland Rewards campaigns.

| Example | What it demonstrates | Maintained by | Tags | Source |
| --- | --- | --- | --- | --- |
| [Clap-to-claim dispenser](/docs/creator/tutorials/examples/rewards/clap-to-claim-dispenser/) | A wearable dispenser that players claim by doing the clap emote in a trigger area. | Decentraland | `rewards`, `wearables`, `triggers`, `emotes` | [Open scene](https://github.com/decentraland/sdk7-goerli-plaza/tree/main/clap-claim-dispenser) |
| [Reward claim dispenser](/docs/creator/tutorials/examples/rewards/reward-claim-dispenser/) | Wearable dispensers and airdrop crates that claim rewards from a Rewards campaign. | Decentraland | `rewards`, `wearables`, `captcha`, `ui` | [Open scene](https://github.com/decentraland/sdk7-goerli-plaza/tree/main/reward-claim) |
<!-- example-scenes-catalog:end -->

## For AI-assisted creation

Give your coding agent the link to the closest example along with a clear request. For example: "Use the Multiplayer Server leaderboard scene as a reference and build a score board for my game."

Install the [SDK Skills](https://github.com/decentraland/sdk-skills) first so the agent has up-to-date SDK7 guidance.

For a broader guide to working with agents, see [Vibe Coding with AI](/docs/creator/sdk7/getting-started/vibe-coding/).

## Legacy references

[Awesome Repository](https://github.com/decentraland-scenes/Awesome-Repository) is useful for historical SDK6 material, but it is not a canonical SDK7 source. Prefer the SDK7 examples above for new projects.
