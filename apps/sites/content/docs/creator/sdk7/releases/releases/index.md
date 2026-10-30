> What changed in the SDK and the Creator Hub over the past year, filtered to the changes that affect creators.

# Release Notes

<!--
Maintainer notes (not rendered). This page is extended by the weekly sync-skills job and by hand.

- Two sections, "## SDK" and "## Creator Hub", newest version first. One "### <version> (<Mon D, YYYY>)" heading per release. Combine releases published the same day with "### 7.20.2 and 7.20.3 (Feb 23, 2026)" when one alone would be near-empty.
- Sources: GitHub releases of decentraland/js-sdk-toolchain and decentraland/creator-hub. Creator Hub app releases (0.x tags) fold in the @dcl/inspector and @dcl/asset-packs prerelease notes that shipped since the previous app release. Items are listed under the app version that first shipped them.
- Keep only changes creators can notice: new components or APIs, editor features, smart items, CLI and publishing behavior, user-visible fixes. Drop CI, tests, tracking/analytics events, dependency and version bumps, refactors, docs-only PRs, and feature-then-revert pairs. Skip a release entirely if nothing survives the filter.
- Format: "* **Short title**: one plain sentence. [#N](https://github.com/decentraland/<repo>/pull/N)". Features get their own bullet, with a relative link to the docs page when one exists. Fixes are condensed into a single "* **Fixes**: a [#N](/docs/creator/sdk7/releases/.../); b [#N](/docs/creator/sdk7/releases/.../)." bullet per release.
- Every item links to its PR. If a release note has no PR, link the release page instead. No em-dashes.
-->

This page lists the changes to the [SDK](#sdk) and the [Creator Hub](#creator-hub) from the past year that affect how you build scenes. Internal work like build tooling, tests, tracking and refactors is left out. Every item links to its pull request on GitHub, where you can find more details.

For the complete, unfiltered notes see the [SDK releases](https://github.com/decentraland/js-sdk-toolchain/releases) and the [Creator Hub releases](https://github.com/decentraland/creator-hub/releases) on GitHub.


> [!NOTE]
> Creator Hub releases bundle the changes from the `@dcl/inspector` (the scene editor) and `@dcl/asset-packs` (smart items) packages. Those changes are listed under the Creator Hub version that first shipped them.


## SDK

### 7.29.0 (Sep 15, 2026)

* **Stacking order between UI modules**: a `zIndex` renderer option decides which UI module draws in front of which. Without it, modules stack in the order they first appear, with the main UI at the back. See [Stacking order between UI modules](/docs/creator/sdk7/2d-ui/onscreen-ui/#stacking-order-between-ui-modules). [#1607](https://github.com/decentraland/js-sdk-toolchain/pull/1607)
* **Fixes**: removing a pointer event handler now removes the entry it created, so a disabled UI button stops advertising a click it can no longer handle, and re-registering handlers no longer piles up duplicate entries. [#1577](https://github.com/decentraland/js-sdk-toolchain/pull/1577)

### 7.28.0 (Sep 10, 2026)

* **Pointer events use camera distance**: `maxCameraDistance` replaces `maxPlayerDistance` on pointer events. The old field still works but is deprecated. Hover hints and clicks now depend on how far the camera is from the item, which matches what the player sees. [#1560](https://github.com/decentraland/js-sdk-toolchain/pull/1560)
* **AvatarNametag component**: customize the name label shown over avatars. [#1600](https://github.com/decentraland/js-sdk-toolchain/pull/1600)
* **Teleport to a realm**: `teleportTo()` can target a specific realm. [#1601](https://github.com/decentraland/js-sdk-toolchain/pull/1601)
* **Fixes**: a disabled UI `Button` no longer dims the scene's colors [#1578](https://github.com/decentraland/js-sdk-toolchain/pull/1578); one entity's pointer input no longer hides another's in the same frame [#1576](https://github.com/decentraland/js-sdk-toolchain/pull/1576); unwired `ActionCallback` parameters in Script smart items resolve to `undefined` instead of failing [#1605](https://github.com/decentraland/js-sdk-toolchain/pull/1605); composites are instanced onto the root entity properly [#1587](https://github.com/decentraland/js-sdk-toolchain/pull/1587); optional component fields set to `0` or `false` are now saved [#1582](https://github.com/decentraland/js-sdk-toolchain/pull/1582); components defined from a primitive or array schema can be read back [#1593](https://github.com/decentraland/js-sdk-toolchain/pull/1593); several serialization and networking robustness fixes [#1592](https://github.com/decentraland/js-sdk-toolchain/pull/1592), [#1595](https://github.com/decentraland/js-sdk-toolchain/pull/1595), [#1568](https://github.com/decentraland/js-sdk-toolchain/pull/1568), [#1570](https://github.com/decentraland/js-sdk-toolchain/pull/1570).

### 7.27.0 (Aug 27, 2026)

* **Event helper improvements**: the pointer, trigger, audio and other event helper systems now have unique names and consistent behavior. [#1536](https://github.com/decentraland/js-sdk-toolchain/pull/1536)
* **Loading screen state**: `EngineInfo` tells you whether the loading screen is still visible. [#1555](https://github.com/decentraland/js-sdk-toolchain/pull/1555)
* **Faster builds**: project files are processed concurrently when building and deploying. [#1481](https://github.com/decentraland/js-sdk-toolchain/pull/1481)
* **Fixes**: click events are detected for all players, not only the first [#1468](https://github.com/decentraland/js-sdk-toolchain/pull/1468); timers keep working after a callback throws [#1470](https://github.com/decentraland/js-sdk-toolchain/pull/1470); unmounted React ECS UI entities are released, fixing a memory leak [#1471](https://github.com/decentraland/js-sdk-toolchain/pull/1471); observable subscriptions no longer interfere with each other [#1467](https://github.com/decentraland/js-sdk-toolchain/pull/1467); the sign-in step when deploying forwards headers correctly [#1551](https://github.com/decentraland/js-sdk-toolchain/pull/1551); renderer-reserved entity IDs are never generated or recycled [#1544](https://github.com/decentraland/js-sdk-toolchain/pull/1544), [#1460](https://github.com/decentraland/js-sdk-toolchain/pull/1460).

### 7.26.0 (Aug 13, 2026)

* **TouchScreenControls component**: show, hide and customize the on-screen controls on mobile. See [On-screen Controls](/docs/creator/sdk7/interactivity/touch-screen-controls/). [#1441](https://github.com/decentraland/js-sdk-toolchain/pull/1441), [#1510](https://github.com/decentraland/js-sdk-toolchain/pull/1510)
* **Open Explorer UI from your scene**: the `openExplorerUi()` restricted action opens the map, backpack, settings and other explorer screens, and `ExplorerUiEventsResult` reports when they close. [#1503](https://github.com/decentraland/js-sdk-toolchain/pull/1503), [#1511](https://github.com/decentraland/js-sdk-toolchain/pull/1511)
* **Audio events and emote state**: the new `audioEventSystem` reacts to audio playback events, and `AvatarEmoteCommand` exposes the emote `state`. [#1530](https://github.com/decentraland/js-sdk-toolchain/pull/1530)
* **UI screen defaults**: UI uses a default virtual screen and screen inset area, and device pixel ratio no longer affects layout. [#1489](https://github.com/decentraland/js-sdk-toolchain/pull/1489)
* **Bevy Web preview**: the browser preview uses the Bevy Web explorer instead of the legacy web explorer. [#1502](https://github.com/decentraland/js-sdk-toolchain/pull/1502)
* **Optimized assets in preview**: the preview can load your models as asset bundles, matching how they look once published. See [Preview with optimized assets](/docs/creator/sdk7/getting-started/preview-scene/#preview-with-optimized-assets). [#1539](https://github.com/decentraland/js-sdk-toolchain/pull/1539)
* **Fixes**: `npm run start` stays alive when the initial build fails, so you can fix the error and it rebuilds [#1501](https://github.com/decentraland/js-sdk-toolchain/pull/1501); busy ports are detected correctly [#1542](https://github.com/decentraland/js-sdk-toolchain/pull/1542).

### 7.24.5 (Jul 15, 2026)

* **Billboard target**: a `Billboard` can face a target entity instead of the camera. [#1445](https://github.com/decentraland/js-sdk-toolchain/pull/1445)
* **Smaller scene bundles**: a second copy of `@dcl/ecs` is no longer bundled into every scene. [#1450](https://github.com/decentraland/js-sdk-toolchain/pull/1450)
* **Fix**: `MessageBus` messages are sent in order. [#1462](https://github.com/decentraland/js-sdk-toolchain/pull/1462)

### 7.24.4 (Jul 3, 2026)

* **InteractableArea for React ECS**: new UI component for on-screen interactable areas. [#1411](https://github.com/decentraland/js-sdk-toolchain/pull/1411)

### 7.24.3 (Jul 1, 2026)

* **No more 45x45 parcel limit** for scenes. [#1429](https://github.com/decentraland/js-sdk-toolchain/pull/1429)
* **Double jump and glide**: `AvatarLocomotionSettings` has fields to toggle double jump and gliding. [#1402](https://github.com/decentraland/js-sdk-toolchain/pull/1402)
* **Fix**: the UI virtual screen accounts for device pixel ratio. [#1433](https://github.com/decentraland/js-sdk-toolchain/pull/1433)

### 7.24.2 (Jun 25, 2026)

* **Fixes**: a re-arming `setTimeout` no longer hangs the scene after a long frame [#1420](https://github.com/decentraland/js-sdk-toolchain/pull/1420); React ECS keeps an optional prop that is set for the first time [#1422](https://github.com/decentraland/js-sdk-toolchain/pull/1422); "Premature close" errors during deploy and other CLI requests are gone [#1428](https://github.com/decentraland/js-sdk-toolchain/pull/1428).

### 7.24.1 (Jun 12, 2026)

* **Player collision layer**: a new collision layer for the main player. [#1414](https://github.com/decentraland/js-sdk-toolchain/pull/1414)
* **Fix**: retriggering an `AudioSource` no longer plays duplicated sounds. [#1382](https://github.com/decentraland/js-sdk-toolchain/pull/1382)

### 7.24.0 (Jun 4, 2026)

* **ScreenInsetArea**: React ECS component that keeps UI inside the safe area on mobile. See [Mobile Safe Area](/docs/creator/build-for-mobile/develop/safe-area/). [#1386](https://github.com/decentraland/js-sdk-toolchain/pull/1386)
* **Composite provider**: `getCompositeProvider()` and `setCompositeProvider()` on the engine, for advanced control of how composites load. [#1333](https://github.com/decentraland/js-sdk-toolchain/pull/1333)

### 7.23.2 (May 13, 2026)

* **Fix**: trigger areas no longer spam network messages while a player stays inside. [#1397](https://github.com/decentraland/js-sdk-toolchain/pull/1397)

### 7.23.1 (May 5, 2026)

* **Audio analysis**: read live data from playing audio sources to drive effects. [#1256](https://github.com/decentraland/js-sdk-toolchain/pull/1256)

### 7.23.0 (May 5, 2026)

* **AvatarLocomotionSettings component**: control the player's walk, run and jump settings. [#1389](https://github.com/decentraland/js-sdk-toolchain/pull/1389)
* **Modifier input action**: `IA_MODIFIER` is available in the input commands list. [#1390](https://github.com/decentraland/js-sdk-toolchain/pull/1390)

### 7.22.5 (Apr 23, 2026)

* **Smaller bundles for code-only scenes**: the smart items library is only bundled when the scene uses a composite. [#1381](https://github.com/decentraland/js-sdk-toolchain/pull/1381)

### 7.22.4 (Apr 16, 2026)

* **Sourcemaps excluded from deployments** via `.dclignore`. [#1373](https://github.com/decentraland/js-sdk-toolchain/pull/1373)
* **Particle system bursts** property updated. [#1375](https://github.com/decentraland/js-sdk-toolchain/pull/1375)

### 7.22.3 (Apr 8, 2026)

* **ParticleSystem component**: emit particles from an entity. See [Particle System](/docs/creator/sdk7/3d-essentials/particle-system/). [#1369](https://github.com/decentraland/js-sdk-toolchain/pull/1369)
* **Less network traffic**: writing an unchanged value to a mutable component no longer sends a message. [#1359](https://github.com/decentraland/js-sdk-toolchain/pull/1359)
* **Clearer deploy errors** when the World doesn't exist, you lack permissions, or a catalyst is unreachable. [#1361](https://github.com/decentraland/js-sdk-toolchain/pull/1361), [#1370](https://github.com/decentraland/js-sdk-toolchain/pull/1370)

### 7.22.2 (Mar 26, 2026)

* **Proximity interactions**: react to the player approaching an entity. See [Proximity Events](/docs/creator/sdk7/interactivity/button-events/proximity-events/). [#1356](https://github.com/decentraland/js-sdk-toolchain/pull/1356)

### 7.22.1 (Mar 26, 2026)

* **Updated sign-in page** used when deploying from the CLI. [#1362](https://github.com/decentraland/js-sdk-toolchain/pull/1362)

### 7.22.0 (Mar 25, 2026)

* **Physics force and impulse**: push the player with forces and impulses. See [Player Physics](/docs/creator/sdk7/interactivity/player-physics/). [#1338](https://github.com/decentraland/js-sdk-toolchain/pull/1338)
* **Multi-scene Worlds**: deploy several scenes to the same World. [#1355](https://github.com/decentraland/js-sdk-toolchain/pull/1355)
* **Deploy from CI** pipelines. [#1357](https://github.com/decentraland/js-sdk-toolchain/pull/1357)
* **Fixes**: reordering keyed lists in React ECS [#1351](https://github.com/decentraland/js-sdk-toolchain/pull/1351); action types are excluded from network sync [#1354](https://github.com/decentraland/js-sdk-toolchain/pull/1354).

### 7.21.0 (Mar 10, 2026)

* **Disable double jump and gliding** in your scene. [#1323](https://github.com/decentraland/js-sdk-toolchain/pull/1323)
* **Tween MoveRotateScale mode**: animate position, rotation and scale in a single tween. [#1339](https://github.com/decentraland/js-sdk-toolchain/pull/1339)
* **Platform detection**: know whether the scene runs on desktop, web or mobile. See [Detect the platform](/docs/creator/build-for-mobile/develop/detect-platform/). [#1340](https://github.com/decentraland/js-sdk-toolchain/pull/1340)

### 7.20.4 (Mar 3, 2026)

* **Multi-instance preview**: run several player instances against one preview to test multiplayer. [#1336](https://github.com/decentraland/js-sdk-toolchain/pull/1336)
* **Fixes**: entity ID collisions [#1332](https://github.com/decentraland/js-sdk-toolchain/pull/1332); `UiInput` no longer echoes its value back [#1331](https://github.com/decentraland/js-sdk-toolchain/pull/1331).

### 7.20.2 and 7.20.3 (Feb 23, 2026)

* **Mobile QR includes the position** to spawn at. [#1328](https://github.com/decentraland/js-sdk-toolchain/pull/1328)
* **Fixes**: values with identical timestamps (for example pointer event results) are ordered correctly [#1322](https://github.com/decentraland/js-sdk-toolchain/pull/1322); mobile preview endpoint fixed ([7.20.3 release](https://github.com/decentraland/js-sdk-toolchain/releases/tag/7.20.3)).

### 7.20.1 (Feb 16, 2026)

* **Better flow when deploying to a multi-scene World.** [#1317](https://github.com/decentraland/js-sdk-toolchain/pull/1317)

### 7.20.0 (Feb 5, 2026)

* **Asset pre-load**: mark assets to load before they're needed. [#1287](https://github.com/decentraland/js-sdk-toolchain/pull/1287)

### 7.19.0 (Jan 29, 2026)

* **SDK helper functions**: shortcuts for common tasks like delaying an instruction. [#1275](https://github.com/decentraland/js-sdk-toolchain/pull/1275)
* **Pointer event max player distance**: limit how far a player can be to interact. [#1293](https://github.com/decentraland/js-sdk-toolchain/pull/1293)

### 7.18.0 (Jan 26, 2026)

* **Multiple UI renderers**: `addUiRenderer()` and `removeUiRenderer()` on the React ECS renderer. [#1285](https://github.com/decentraland/js-sdk-toolchain/pull/1285)
* **Automatic UI scale factor** in React ECS. [#1290](https://github.com/decentraland/js-sdk-toolchain/pull/1290)
* **Script instances from code**: call the Script components of your smart items from anywhere in your code, and trigger editor-configured actions with `ActionCallback`. [#1284](https://github.com/decentraland/js-sdk-toolchain/pull/1284)
* **Markdown files excluded from deployments.** [#1282](https://github.com/decentraland/js-sdk-toolchain/pull/1282)

### 7.17.0 (Jan 5, 2026)

* **Build errors show the type-check output.** [#1263](https://github.com/decentraland/js-sdk-toolchain/pull/1263)
* **Mobile preview prints coordinates and a deep link.** [#1268](https://github.com/decentraland/js-sdk-toolchain/pull/1268)

### 7.16.0 (Dec 30, 2025)

* **Visibility propagates to children**: hiding an entity with `VisibilityComponent` also hides its children. [#1262](https://github.com/decentraland/js-sdk-toolchain/pull/1262)
* **Preview on mobile**: run the preview in the mobile app. [#1261](https://github.com/decentraland/js-sdk-toolchain/pull/1261)
* **Bevy Web browser preview** supported. [#1241](https://github.com/decentraland/js-sdk-toolchain/pull/1241)
* **`npm run start` opens the new desktop explorer by default.** [#1264](https://github.com/decentraland/js-sdk-toolchain/pull/1264)
* **Fix**: smart wearable project template downloads again. [#1255](https://github.com/decentraland/js-sdk-toolchain/pull/1255)

### 7.15.0 and 7.15.1 (Dec 8, 2025)

* **Spatial audio** for `AudioSource` and `VideoPlayer`. [#1243](https://github.com/decentraland/js-sdk-toolchain/pull/1243)
* **Script component support** for smart items with code. [#1245](https://github.com/decentraland/js-sdk-toolchain/pull/1245)
* **Fixes**: Windows prompts to download the Desktop Client when it's missing [#1244](https://github.com/decentraland/js-sdk-toolchain/pull/1244); smart items resolve correctly from nested `node_modules` [#1258](https://github.com/decentraland/js-sdk-toolchain/pull/1258).

### 7.12.1 (Nov 3, 2025)

* **`code-to-composite` command**: turn scene code into a composite the editor can open. [#1232](https://github.com/decentraland/js-sdk-toolchain/pull/1232)

### 7.12.0 (Oct 30, 2025)

* **CopyToClipboard restricted action**. [#1234](https://github.com/decentraland/js-sdk-toolchain/pull/1234)

### 7.11.0 (Oct 15, 2025)

* **Clearer errors from the content server** when deploying. [#1217](https://github.com/decentraland/js-sdk-toolchain/pull/1217)
* **`dclcontext` and thumbnails folders excluded from deployments.** [#1227](https://github.com/decentraland/js-sdk-toolchain/pull/1227), [#1229](https://github.com/decentraland/js-sdk-toolchain/pull/1229)

### 7.10.6 (Oct 9, 2025)

* **Tween continuous modes**: keep rotating or moving indefinitely. [#1221](https://github.com/decentraland/js-sdk-toolchain/pull/1221)

### 7.10.5 (Oct 6, 2025)

* **`get-context-files` command**: downloads the AI context docs into your scene so coding assistants know the SDK. [#1220](https://github.com/decentraland/js-sdk-toolchain/pull/1220)

### 7.10.3 (Sep 29, 2025)

* **Trigger area components**: detect players entering, staying in and leaving a region. See [Trigger Areas](/docs/creator/sdk7/3d-essentials/trigger-areas/). [#1210](https://github.com/decentraland/js-sdk-toolchain/pull/1210)
* **Tags component**: label entities and query them by tag. [#1216](https://github.com/decentraland/js-sdk-toolchain/pull/1216)

## Creator Hub

### 0.50.0 (Sep 17, 2026)

* **Assets dropped in the project folder show up on their own**: files copied into the project's `assets` folder from outside the editor appear in **Local Assets** without pressing **Refresh**. See [Import custom assets](/docs/creator/scene-editor/build/import-items/). [#1606](https://github.com/decentraland/creator-hub/pull/1606)
* **Fixes**: **Show QR Code for Mobile** works whatever preview client is selected, and the Bevy (Web) preview reuses its tab [#1608](https://github.com/decentraland/creator-hub/pull/1608); publishing no longer waits for LOD generation, so the **Optimizing** step is gone [#1603](https://github.com/decentraland/creator-hub/pull/1603); the UI Editor no longer leaves an orphan `./ui` import on older scenes [#1609](https://github.com/decentraland/creator-hub/pull/1609).

### 0.49.0 (Sep 16, 2026)

* **Locomotion Settings smart item**: change how fast players move and how high they jump, with sliders instead of code. See [Locomotion settings](/docs/creator/scene-editor/interactivity/smart-items/#locomotion-settings). [#1584](https://github.com/decentraland/creator-hub/pull/1584)
* **AI Assistant supports Cursor and Gemini**, alongside Claude and Codex. See [AI Assistant](/docs/creator/scene-editor/code/ai-assistant/). [#1591](https://github.com/decentraland/creator-hub/pull/1591)
* **Type exact transform amounts**: press a tool key, an axis and a number to move, rotate or scale by a precise amount instead of dragging. **F** is no longer a shortcut for the free gizmo. See [Type an exact amount](/docs/creator/scene-editor/get-started/scene-editor-essentials/#type-an-exact-amount). [#1599](https://github.com/decentraland/creator-hub/pull/1599)
* **Fixes**: the UI Editor no longer fogs the view with full-screen containers, and nested GUI preview values and free placement work [#1590](https://github.com/decentraland/creator-hub/pull/1590); the Animator component shows under the Bevy editor [#1587](https://github.com/decentraland/creator-hub/pull/1587); Bevy editor hot-reload is driven by real build events [#1592](https://github.com/decentraland/creator-hub/pull/1592); AI Assistant follow-up turns work again with Codex [#1600](https://github.com/decentraland/creator-hub/pull/1600); dropdown and kebab menus are no longer clipped [#1595](https://github.com/decentraland/creator-hub/pull/1595); one-click Create Scene, entity names keep consecutive spaces, the scene card menu no longer reflows the grid, asset tooltips sit correctly in search results, and you can drag from the Local Assets tree [#1593](https://github.com/decentraland/creator-hub/pull/1593).

### 0.48.0 (Sep 10, 2026)

* **UI Editor mobile preview**: a real phone frame with an accurate safe area, using the same device presets as the client, plus optional HUD guides (joystick, buttons, chat) you can toggle. [#1579](https://github.com/decentraland/creator-hub/pull/1579)
* **AI Assistant redesign**: cleaner panel, reliable close button, persistent intro message and better settings. See [AI Assistant](/docs/creator/scene-editor/code/ai-assistant/). [#1586](https://github.com/decentraland/creator-hub/pull/1586)
* **Analytics shows median playtime** instead of the average. [#1564](https://github.com/decentraland/creator-hub/pull/1564)
* **Changes**: the Bevy renderer is labeled "experimental" [#1583](https://github.com/decentraland/creator-hub/pull/1583); the scene settings field is now "Creator name" [#1561](https://github.com/decentraland/creator-hub/pull/1561); the asset-bundle delay warning on the publish screen is gone [#1582](https://github.com/decentraland/creator-hub/pull/1582).
* **Fix**: on Windows, AI Assistant turns no longer fail when the command line gets too long. [#1588](https://github.com/decentraland/creator-hub/pull/1588)

### 0.47.0 (Sep 4, 2026)

* **Scene Agent**: an AI assistant built into the editor. It reads your scene, writes SDK7 code, creates and edits entities, components, smart items and scripts live in the viewport, and can run the preview to check its work. It drives your installed Claude or Codex CLI, so there's no API key to set up. [#1499](https://github.com/decentraland/creator-hub/pull/1499)
* **Smart items run on Script components**: each smart item's behavior now lives as code in a `.ts` file inside the item's folder, with its main functions exposed as actions. You can read and tweak it. [#1354](https://github.com/decentraland/creator-hub/pull/1354)
* **Slider inputs for Script smart items**: a number parameter can declare a range and shows as a slider. See [Using the Script Component](/docs/creator/scene-editor/code/script-component/). [#1513](https://github.com/decentraland/creator-hub/pull/1513)
* **UI Editor**: a node's own position (pinned or in-flow) is independent from how it arranges its children, and nodes have Figma-style icons. [#1552](https://github.com/decentraland/creator-hub/pull/1552)
* **One shared tutorials list** on the Scenes sidebar and the Learn page. [#1551](https://github.com/decentraland/creator-hub/pull/1551)
* **Fixes**: Intel Macs launch again [#1545](https://github.com/decentraland/creator-hub/pull/1545); the UI Editor remembers 2D or 3D mode [#1532](https://github.com/decentraland/creator-hub/pull/1532); imported animations default to a weight of 1 [#1557](https://github.com/decentraland/creator-hub/pull/1557); the Script component appears right after GLTF in the entity panel [#1556](https://github.com/decentraland/creator-hub/pull/1556); custom items load under the Bevy renderer [#1555](https://github.com/decentraland/creator-hub/pull/1555).

### 0.46.0 (Aug 27, 2026)

* **Full Screen container** in the UI Editor: a container that fills the whole screen in one click. [#1540](https://github.com/decentraland/creator-hub/pull/1540)

### 0.45.0 (Aug 27, 2026)

* **UI Editor (experimental)**: design your scene's on-screen interface visually on a 2D canvas with a nodes tree and a properties panel. Your `@dcl/react-ecs` code stays the single source of truth. Turn it on in Settings > Experimental. Requires SDK 7.26.0 or later. See [UI Editor](/docs/creator/scene-editor/build/ui-editor/). [#1523](https://github.com/decentraland/creator-hub/pull/1523), [#1522](https://github.com/decentraland/creator-hub/pull/1522), [#1521](https://github.com/decentraland/creator-hub/pull/1521), [#1520](https://github.com/decentraland/creator-hub/pull/1520), [#1524](https://github.com/decentraland/creator-hub/pull/1524)
* **Avatar and Camera Modifier Areas**: change how avatars appear and how the camera behaves inside a region. Add them as components or as smart items. See [Modifier Areas](/docs/creator/scene-editor/interactivity/modifier-areas/). [#1471](https://github.com/decentraland/creator-hub/pull/1471)
* **Friendzone Buildathon 2026 workshops** in Learn > Videos. [#1510](https://github.com/decentraland/creator-hub/pull/1510)
* **Fixes**: publishing to LAND no longer hangs on "Publishing…" [#1496](https://github.com/decentraland/creator-hub/pull/1496); the confirm button works when publishing to a World from the Bevy editor [#1493](https://github.com/decentraland/creator-hub/pull/1493); Analytics lists every deployed scene [#1492](https://github.com/decentraland/creator-hub/pull/1492); new NAMEs and World permissions appear without restarting [#1509](https://github.com/decentraland/creator-hub/pull/1509); the Windows "Creator Hub cannot be closed" update dialog is gone [#1497](https://github.com/decentraland/creator-hub/pull/1497); the More tab drops the Legacy Web Editor card and links Names to the marketplace [#1495](https://github.com/decentraland/creator-hub/pull/1495).

### 0.44.2 (Aug 14, 2026)

* **Analytics**: visitors, retention, visits and engagement for every World and Genesis City scene you own or collaborate on, split by desktop and mobile. See [Scene Analytics](/docs/creator/sdk7/other/scene-analytics/). [#1447](https://github.com/decentraland/creator-hub/pull/1447)
* **Redesigned home screen.** [#1481](https://github.com/decentraland/creator-hub/pull/1481)
* **Fixes**: GLB imports no longer hang on a spinner [#1487](https://github.com/decentraland/creator-hub/pull/1487); the local multiplayer server starts in scene previews again [#1455](https://github.com/decentraland/creator-hub/pull/1455).

### 0.44.0 (Aug 4, 2026)

* **Bevy renderer (experimental)**: an alternative renderer for the editor viewport. [#1330](https://github.com/decentraland/creator-hub/pull/1330), [#1363](https://github.com/decentraland/creator-hub/pull/1363)
* **Optimize Assets preview option**: preview with asset bundles so models look as they will once published. See [Preview with optimized assets](/docs/creator/sdk7/getting-started/preview-scene/#preview-with-optimized-assets). [#1396](https://github.com/decentraland/creator-hub/pull/1396)
* **Explorer MCP toggle** in the preview options. [#1440](https://github.com/decentraland/creator-hub/pull/1440)
* **Entity tree search.** [#1385](https://github.com/decentraland/creator-hub/pull/1385)
* **Rename a project's folder** from the Scenes view. [#1357](https://github.com/decentraland/creator-hub/pull/1357)
* **"Hide Landscape Terrain" scene setting.** [#1383](https://github.com/decentraland/creator-hub/pull/1383)
* **Admin tools reskinned** to the new Decentraland design. [#1360](https://github.com/decentraland/creator-hub/pull/1360)
* **Removed the Test Server publish option.** [#1441](https://github.com/decentraland/creator-hub/pull/1441)
* **Fixes**: the mobile QR preview no longer opens the desktop client [#1452](https://github.com/decentraland/creator-hub/pull/1452); lock and hide toggles in the entity tree can be turned back off [#1429](https://github.com/decentraland/creator-hub/pull/1429); Custom Items include Placeholder component assets [#1287](https://github.com/decentraland/creator-hub/pull/1287); the DCL Cast admin panel auto-opens, keeps slide state and replicates to late joiners [#1358](https://github.com/decentraland/creator-hub/pull/1358); admin tools skip initialization on the authoritative server [#1381](https://github.com/decentraland/creator-hub/pull/1381).

### 0.43.0 (Jul 6, 2026)

* **Simpler sign-in**: a single browser confirmation instead of typing a code. [#1338](https://github.com/decentraland/creator-hub/pull/1338)
* **Fixes**: clear error when a deployment times out [#1355](https://github.com/decentraland/creator-hub/pull/1355); correct parcel count for scenes spanning negative and positive coordinates [#1352](https://github.com/decentraland/creator-hub/pull/1352); "Scene already imported" message when re-importing [#1350](https://github.com/decentraland/creator-hub/pull/1350).

### 0.42.0 (Jun 26, 2026)

* **Particle System component** in the editor. See [Particle System](/docs/creator/sdk7/3d-essentials/particle-system/). [#1311](https://github.com/decentraland/creator-hub/pull/1311)
* **Spawn Entity action**: spawn a smart item from an action chain, instead of pre-placing hidden copies. [#1322](https://github.com/decentraland/creator-hub/pull/1322)
* **Copy and paste component values** from a component's three-dots menu. [#1321](https://github.com/decentraland/creator-hub/pull/1321)
* **Up to 100 collaborators** per World. [#1310](https://github.com/decentraland/creator-hub/pull/1310)
* **Collaborators can unpublish World scenes.** [#1209](https://github.com/decentraland/creator-hub/pull/1209)
* **Changes**: the Teen age rating is removed [#1329](https://github.com/decentraland/creator-hub/pull/1329); a warning explains that live streaming isn't supported in single player mode [#1336](https://github.com/decentraland/creator-hub/pull/1336).
* **Fixes**: Wind Area and Impulse Area react only to the current player [#1323](https://github.com/decentraland/creator-hub/pull/1323); admin tools UI fits mobile screens [#1256](https://github.com/decentraland/creator-hub/pull/1256); controls for several components [#1282](https://github.com/decentraland/creator-hub/pull/1282); inspector startup errors [#1334](https://github.com/decentraland/creator-hub/pull/1334).

### 0.41.1 (Apr 29, 2026)

* **Refresh button** on the top bar restarts the editor, handy after editing the scene with AI tools. [#1312](https://github.com/decentraland/creator-hub/pull/1312)
* **Console auto-scrolls** and supports copying text. [#1305](https://github.com/decentraland/creator-hub/pull/1305)

### 0.41.0 (Apr 27, 2026)

* **Mobile Debug Session**: pair with the mobile client via QR and stream entity state, console logs and performance data into the editor, with tick scrubbing and pause, resume and reload commands. [#1306](https://github.com/decentraland/creator-hub/pull/1306)
* **Speaker Showcase and presentation panel for DCL Cast**: manage live speakers and slides from the Admin Tools. [#1213](https://github.com/decentraland/creator-hub/pull/1213)
* **Entity-level errors** in the entity tree: a red icon flags entities with invalid component values. [#1258](https://github.com/decentraland/creator-hub/pull/1258)
* **"Disable Nearby Voice Chat"** scene restriction. [#1289](https://github.com/decentraland/creator-hub/pull/1289)
* **"View storage" button** for Worlds, to view and edit authoritative server data. [#1307](https://github.com/decentraland/creator-hub/pull/1307)
* **Wind Area and Impulse Area smart items** that push the player. [#1296](https://github.com/decentraland/creator-hub/pull/1296), [#1308](https://github.com/decentraland/creator-hub/pull/1308)
* **Reward smart items**: Balloons, two Wearable dispensers and a Clap claim dispenser that hand out wearables from a rewards campaign. [#1124](https://github.com/decentraland/creator-hub/pull/1124)
* **Fixes**: rotation values no longer drift when leaving the field [#1266](https://github.com/decentraland/creator-hub/pull/1266); the "Fixed Skybox" checkbox no longer flips with other checkboxes [#1254](https://github.com/decentraland/creator-hub/pull/1254); deploying to a World only undeploys scenes whose parcels overlap [#1271](https://github.com/decentraland/creator-hub/pull/1271); duplicates appear next to the original in the tree [#1150](https://github.com/decentraland/creator-hub/pull/1150); updated trigger area model [#1285](https://github.com/decentraland/creator-hub/pull/1285).

### 0.39.1 (Mar 30, 2026)

* **"Show in Places"** checkbox in World settings. [#1264](https://github.com/decentraland/creator-hub/pull/1264)
* **Fixes**: preview shows the error when the scene code is broken instead of failing silently [#1238](https://github.com/decentraland/creator-hub/pull/1238); multi-scene flag validated in the deploy file [#1270](https://github.com/decentraland/creator-hub/pull/1270).

### 0.39.0 (Mar 26, 2026)

* **Fixes**: Windows installer update [#1252](https://github.com/decentraland/creator-hub/pull/1252); multi-scene deploy [#1262](https://github.com/decentraland/creator-hub/pull/1262); smart items that weren't working [#1260](https://github.com/decentraland/creator-hub/pull/1260); trigger and click areas show a pointer mesh [#1255](https://github.com/decentraland/creator-hub/pull/1255).

### 0.38.0 (Mar 20, 2026)

* **Fixes**: asset packs scroll again [#1239](https://github.com/decentraland/creator-hub/pull/1239); audio files kept when spawning a custom smart item in a new scene [#1234](https://github.com/decentraland/creator-hub/pull/1234).

### 0.37.0 (Mar 16, 2026)

* **Embedded debug console** as a tab in the Assets panel. [#1200](https://github.com/decentraland/creator-hub/pull/1200)
* **Fixes**: custom items keep their texture files [#1217](https://github.com/decentraland/creator-hub/pull/1217); gizmos work on the first selection [#1218](https://github.com/decentraland/creator-hub/pull/1218); Show Image action defaults to center alignment [#1221](https://github.com/decentraland/creator-hub/pull/1221); video source copy no longer mentions Vimeo only [#1216](https://github.com/decentraland/creator-hub/pull/1216).

### 0.36.0 (Mar 10, 2026)

* **Fixes**: spawn point selection in the entity tree [#1199](https://github.com/decentraland/creator-hub/pull/1199); "Restricted" removed from World content ratings [#1205](https://github.com/decentraland/creator-hub/pull/1205); Manage Published only lists Worlds with published scenes [#1208](https://github.com/decentraland/creator-hub/pull/1208); deployment error when the World limit is exceeded [#1210](https://github.com/decentraland/creator-hub/pull/1210).

### 0.35.0 (Mar 5, 2026)

* **Spawn points visualization** in the editor. See [Spawn Areas](/docs/creator/scene-editor/build/spawn-areas/). [#1129](https://github.com/decentraland/creator-hub/pull/1129)
* **Dynamic asset catalog**: the latest items load at runtime, with an SDK compatibility check. [#1172](https://github.com/decentraland/creator-hub/pull/1172)
* **Multi-instance preview** toggle to test multiplayer. Requires SDK 7.20.4 or later. [#1187](https://github.com/decentraland/creator-hub/pull/1187)
* **World permissions Access tab**, and Multiplayer and Skybox settings back in World Settings. [#1186](https://github.com/decentraland/creator-hub/pull/1186), [#1189](https://github.com/decentraland/creator-hub/pull/1189)
* **UI refresh** of colors, component styles and the entity inspector. [#1178](https://github.com/decentraland/creator-hub/pull/1178)
* **Fixes**: TextShape initializes correctly [#1194](https://github.com/decentraland/creator-hub/pull/1194) and renders without clipping [#1185](https://github.com/decentraland/creator-hub/pull/1185); duplicated ground tiles [#1179](https://github.com/decentraland/creator-hub/pull/1179); scene layout updates in advanced mode [#1181](https://github.com/decentraland/creator-hub/pull/1181); selection highlight no longer lags or turns objects black [#1196](https://github.com/decentraland/creator-hub/pull/1196); managed projects load [#1182](https://github.com/decentraland/creator-hub/pull/1182); Scene Admin UI scales on all screens [#1190](https://github.com/decentraland/creator-hub/pull/1190).

### 0.34.0 (Feb 25, 2026)

* **Multi-scene Worlds**: publish several scenes to one World. [#1089](https://github.com/decentraland/creator-hub/pull/1089), [#1163](https://github.com/decentraland/creator-hub/pull/1163)
* **Manage Worlds**: World settings and a manage page. [#1132](https://github.com/decentraland/creator-hub/pull/1132), [#1049](https://github.com/decentraland/creator-hub/pull/1049), [#1112](https://github.com/decentraland/creator-hub/pull/1112)
* **Collaborator permissions** for Worlds. [#1128](https://github.com/decentraland/creator-hub/pull/1128), [#1083](https://github.com/decentraland/creator-hub/pull/1083), [#1125](https://github.com/decentraland/creator-hub/pull/1125)
* **Unpublish a scene from a World.** [#1123](https://github.com/decentraland/creator-hub/pull/1123)
* **Placeholder component**: invisible entities for sit spots, trigger areas and the like, without affecting scene metrics. [#1154](https://github.com/decentraland/creator-hub/pull/1154)
* **Log and Delete actions** for smart items. [#1131](https://github.com/decentraland/creator-hub/pull/1131)
* **Fixes**: trigger areas only activate for your own player [#1142](https://github.com/decentraland/creator-hub/pull/1142); devtools for scene web traffic work again [#1109](https://github.com/decentraland/creator-hub/pull/1109); Virtual Camera duration and self-selection in "Change camera" [#1160](https://github.com/decentraland/creator-hub/pull/1160); scale gizmo [#1082](https://github.com/decentraland/creator-hub/pull/1082); Photo Wall smart item [#1110](https://github.com/decentraland/creator-hub/pull/1110); the "Custom Code" warning is more accurate [#1171](https://github.com/decentraland/creator-hub/pull/1171); World access problems [#1133](https://github.com/decentraland/creator-hub/pull/1133); scene thumbnails [#1168](https://github.com/decentraland/creator-hub/pull/1168); unused Save icon and "Slow connection" warning removed [#1152](https://github.com/decentraland/creator-hub/pull/1152), [#1169](https://github.com/decentraland/creator-hub/pull/1169).

### 0.33.1 (Feb 18, 2026)

* **Preview on the mobile app**, configurable in the preview settings. [#1121](https://github.com/decentraland/creator-hub/pull/1121)
* **Script component tooltips** from comments in the script file, and an improved template. [#1130](https://github.com/decentraland/creator-hub/pull/1130), [#1019](https://github.com/decentraland/creator-hub/pull/1019)
* **Font selection** in the TextShape component. [#1096](https://github.com/decentraland/creator-hub/pull/1096)
* **Fixes**: rotation gizmo [#1081](https://github.com/decentraland/creator-hub/pull/1081); editing light smart items [#1139](https://github.com/decentraland/creator-hub/pull/1139); false "signal is aborted" error when publishing [#1144](https://github.com/decentraland/creator-hub/pull/1144); gizmo no longer switches to free move after duplicating [#1088](https://github.com/decentraland/creator-hub/pull/1088); Clap Meter and Photo Wall smart items [#1101](https://github.com/decentraland/creator-hub/pull/1101).

### 0.33.0 (Feb 4, 2026)

* **Script methods as actions**: the Action type dropdown includes methods from Script components. [#1026](https://github.com/decentraland/creator-hub/pull/1026), [#1071](https://github.com/decentraland/creator-hub/pull/1071)
* **`isAdmin` function** to know if the player is a scene admin. [#1037](https://github.com/decentraland/creator-hub/pull/1037)
* **Worlds storage page.** [#1036](https://github.com/decentraland/creator-hub/pull/1036)
* **Offline mode** with a "No connection" page. [#991](https://github.com/decentraland/creator-hub/pull/991)
* **Fixes**: asset search considers tags and descriptions [#1022](https://github.com/decentraland/creator-hub/pull/1022); scale gizmo responds to left-right drag and can't set size to 0 [#1065](https://github.com/decentraland/creator-hub/pull/1065); proportional scaling with values below 1 [#1076](https://github.com/decentraland/creator-hub/pull/1076); lock state isn't inherited by children [#1085](https://github.com/decentraland/creator-hub/pull/1085); Photo Mural supports pictures from Worlds [#1074](https://github.com/decentraland/creator-hub/pull/1074); image-only custom items create their folder [#1033](https://github.com/decentraland/creator-hub/pull/1033); decimal values in sliders [#1075](https://github.com/decentraland/creator-hub/pull/1075).

### 0.32.1 (Jan 21, 2026)

* **New actions**: Freeze Player, Unfreeze Player, Face Item, Change Collisions, Change Skybox, Reset Skybox, plus a Camera smart item for "Change Camera". [#997](https://github.com/decentraland/creator-hub/pull/997), [#989](https://github.com/decentraland/creator-hub/pull/989)
* **Change multiple States at once** by selecting several items. [#975](https://github.com/decentraland/creator-hub/pull/975)
* **Republish shortcut**: redeploy to the same World or coordinates without going through the configuration menus. [#1003](https://github.com/decentraland/creator-hub/pull/1003)
* **New scripted smart items**: Clap Meter, Wearables Scanner, Zombie and Photo Mural. The Padlock can trigger "Open" on another entity. [#1008](https://github.com/decentraland/creator-hub/pull/1008)
* **`ActionRef` and `ActionCallback` input types** for Scripts, to handle another item's actions from code. [#1054](https://github.com/decentraland/creator-hub/pull/1054)
* **Tooltips** across components, actions and smart items. [#1009](https://github.com/decentraland/creator-hub/pull/1009)
* **Manage Projects page.** [#1032](https://github.com/decentraland/creator-hub/pull/1032)
* **Fixes**: video screen models are double-sided [#1007](https://github.com/decentraland/creator-hub/pull/1007); scene metrics count triangles, materials and textures correctly [#1020](https://github.com/decentraland/creator-hub/pull/1020).

### 0.32.0 (Jan 5, 2026)

* **Edit a component on several entities at once**: Tag, Material, AudioSource, AudioStream, MeshCollider, TextShape, Visibility and LightSource. [#949](https://github.com/decentraland/creator-hub/pull/949), [#950](https://github.com/decentraland/creator-hub/pull/950), [#958](https://github.com/decentraland/creator-hub/pull/958), [#959](https://github.com/decentraland/creator-hub/pull/959), [#960](https://github.com/decentraland/creator-hub/pull/960), [#963](https://github.com/decentraland/creator-hub/pull/963), [#964](https://github.com/decentraland/creator-hub/pull/964)
* **Creator field** in the entity inspector. [#944](https://github.com/decentraland/creator-hub/pull/944)
* **New app settings UI.** [#988](https://github.com/decentraland/creator-hub/pull/988)
* **Clean Unused Assets**: select all, intermediate state and undo. [#993](https://github.com/decentraland/creator-hub/pull/993)
* **More components** available in the editor. [#999](https://github.com/decentraland/creator-hub/pull/999)
* **Fixes**: hidden items stay invisible [#974](https://github.com/decentraland/creator-hub/pull/974); transparent materials [#976](https://github.com/decentraland/creator-hub/pull/976); axis indicator shows north [#978](https://github.com/decentraland/creator-hub/pull/978); deploying with an expired identity [#981](https://github.com/decentraland/creator-hub/pull/981); publish modal no longer closes on backdrop click [#982](https://github.com/decentraland/creator-hub/pull/982); deployment error display [#985](https://github.com/decentraland/creator-hub/pull/985); smart item dropdown overlap [#995](https://github.com/decentraland/creator-hub/pull/995); undo no longer creates an undeletable template [#998](https://github.com/decentraland/creator-hub/pull/998); position gizmo performance [#972](https://github.com/decentraland/creator-hub/pull/972).

### 0.31.0 and 0.31.1 (Dec 16, 2025)

* **Planar position gizmos.** [#943](https://github.com/decentraland/creator-hub/pull/943)
* **Camera pan** and an **axis helper** in the viewport. [#952](https://github.com/decentraland/creator-hub/pull/952), [#951](https://github.com/decentraland/creator-hub/pull/951)
* **DCL Cast** smart item for streaming video into a scene. [#957](https://github.com/decentraland/creator-hub/pull/957)
* **New actions** in smart items. [#955](https://github.com/decentraland/creator-hub/pull/955)
* **Deselect by clicking an empty area.** [#961](https://github.com/decentraland/creator-hub/pull/961)
* **"Link copied" feedback** on the share button. [#965](https://github.com/decentraland/creator-hub/pull/965)
* **Fixes**: importing multiple files [#931](https://github.com/decentraland/creator-hub/pull/931); final position when scaling several objects [#962](https://github.com/decentraland/creator-hub/pull/962).

### 0.30.0 (Dec 9, 2025)

* **Script component**: attach custom code to any item. See [Using the Script Component](/docs/creator/scene-editor/code/script-component/). [#886](https://github.com/decentraland/creator-hub/pull/886)
* **Press F** to center the camera on the selected entity. [#948](https://github.com/decentraland/creator-hub/pull/948)
* **Warning when a scene has custom code.** [#903](https://github.com/decentraland/creator-hub/pull/903)
* **Custom assets tab shows a tutorial** when empty. [#925](https://github.com/decentraland/creator-hub/pull/925)
* **Fixes**: rotate multiple items together [#946](https://github.com/decentraland/creator-hub/pull/946); Add Component hidden on scene, camera and player entities [#923](https://github.com/decentraland/creator-hub/pull/923); clearer errors when a scene fails to load [#921](https://github.com/decentraland/creator-hub/pull/921); Windows prompts to download the launcher [#899](https://github.com/decentraland/creator-hub/pull/899); all duplicates stay selected [#929](https://github.com/decentraland/creator-hub/pull/929); video player placeholder orientation [#909](https://github.com/decentraland/creator-hub/pull/909); duplicate numbering [#933](https://github.com/decentraland/creator-hub/pull/933); Custom Items folder is created when missing [#940](https://github.com/decentraland/creator-hub/pull/940); Clean Unused Assets removes empty folders [#924](https://github.com/decentraland/creator-hub/pull/924); publishing scenes with custom code [#941](https://github.com/decentraland/creator-hub/pull/941); checkbox fields in the smart item basic view [#922](https://github.com/decentraland/creator-hub/pull/922).

### 0.29.3 (Nov 20, 2025)

* **Fixes**: newly imported scenes no longer get stuck loading [#908](https://github.com/decentraland/creator-hub/pull/908); no more multiple Finder or Explorer windows [#906](https://github.com/decentraland/creator-hub/pull/906); deployment flow [#896](https://github.com/decentraland/creator-hub/pull/896); video screen smart items [#898](https://github.com/decentraland/creator-hub/pull/898).

### 0.29.1 and 0.29.2 (Nov 13, 2025)

* **Fixes**: Video Player rendering and its entry in the component list, with drag and drop [#872](https://github.com/decentraland/creator-hub/pull/872), [#873](https://github.com/decentraland/creator-hub/pull/873); NFT smart item [#885](https://github.com/decentraland/creator-hub/pull/885); Clean Unused Assets no longer breaks models or thumbnails [#887](https://github.com/decentraland/creator-hub/pull/887), [#882](https://github.com/decentraland/creator-hub/pull/882); the editor grays out while dependencies update [#891](https://github.com/decentraland/creator-hub/pull/891).

### 0.29.0 (Nov 5, 2025)

* **Clean Unused Assets** tool. [#857](https://github.com/decentraland/creator-hub/pull/857)
* **Scene help panel**: a scene's markdown help file shows in the editor, useful for templates. [#833](https://github.com/decentraland/creator-hub/pull/833)
* **Shortcut to open the scene folder** in Finder or Explorer. [#853](https://github.com/decentraland/creator-hub/pull/853)
* **Fixes**: template downloads [#845](https://github.com/decentraland/creator-hub/pull/845); NFT smart item contract field [#839](https://github.com/decentraland/creator-hub/pull/839); camera and player entities can't be parented [#855](https://github.com/decentraland/creator-hub/pull/855).

### 0.28.0 (Oct 23, 2025)

* **Lights and node modifiers** in the editor. See [Lights](/docs/creator/sdk7/3d-essentials/lights/). [#813](https://github.com/decentraland/creator-hub/pull/813)
* **Fix**: duplicated items can be moved. [#815](https://github.com/decentraland/creator-hub/pull/815)

### 0.27.5 (Oct 15, 2025)

* **Entity tagging.** [#790](https://github.com/decentraland/creator-hub/pull/790)
* **AI context files** are downloaded into each scene when created or updated, so coding assistants know the SDK. [#817](https://github.com/decentraland/creator-hub/pull/817)
* **Submit an event** card in the More section. [#811](https://github.com/decentraland/creator-hub/pull/811)
* **Fixes**: animation weight accepts decimals [#826](https://github.com/decentraland/creator-hub/pull/826); sound and model thumbnails [#822](https://github.com/decentraland/creator-hub/pull/822), [#804](https://github.com/decentraland/creator-hub/pull/804); blank preview debugger [#821](https://github.com/decentraland/creator-hub/pull/821); assets refresh after import [#809](https://github.com/decentraland/creator-hub/pull/809); deploys through the linker server [#812](https://github.com/decentraland/creator-hub/pull/812).

### 0.25.3 (Oct 6, 2025)

* **Chrome DevTools** available from the Creator Hub for debugging scenes. [#766](https://github.com/decentraland/creator-hub/pull/766)
* **Fix**: creating new scenes. [#794](https://github.com/decentraland/creator-hub/pull/794)

### 0.24.1 (Sep 17, 2025)

* **Default code editor setting**: pick VS Code, Cursor or another installed editor. [#751](https://github.com/decentraland/creator-hub/pull/751)
* **Delete files from disk** when deleting a project, as an option. [#787](https://github.com/decentraland/creator-hub/pull/787)
* **Fixes**: textures can be removed and load from URLs [#772](https://github.com/decentraland/creator-hub/pull/772), [#779](https://github.com/decentraland/creator-hub/pull/779); canvas resize when deleting an entity [#770](https://github.com/decentraland/creator-hub/pull/770).
