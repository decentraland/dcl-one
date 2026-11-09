> Features available on the Decentraland desktop client that are not yet supported in the mobile app, and known cross-platform inconsistencies.

# Missing Features


> [!NOTE]
> This page tracks the feature gap between the Decentraland desktop (Unity) client and the mobile app. It is sourced from the [godot-explorer feature parity tracker](https://github.com/decentraland/godot-explorer/issues/2402) and updated regularly. ETAs are estimates and subject to change.
>
> Last reviewed: **October 2026** (mobile app v1.15.0).


## Recently Shipped

These are no longer gaps as of mobile app **v1.15.0**, which is rolling out to the app stores in October 2026:

- [`stopEmote` restricted action](/docs/creator/sdk7/interactivity/avatars/avatar-animations/#animate-only-the-upper-body). Upper-body emotes started by a scene also pause when the player leaves the scene, resume on return, and stop when the scene unloads — v1.15.0
- [`Tween` `MoveRotateScale` mode](/docs/creator/sdk7/3d-essentials/move-entities/#simultaneous-tweens) — v1.15.0
- Other players' positions are reported to scenes relative to the scene, like the local player's (`getPlayer({ userId }).position`, `onEnterScene`, items attached to other players) — v1.15.0
- Player movement, first Unity-parity pass: capsule size, step height, jump arc and gravity, run speed and air control — v1.15.0
- Downward raycasts from the player report the correct `hitDistance` on sloped geometry — v1.15.0
- Upper-body emotes are seen as upper-body by other mobile players — v1.15.0

## SDK Features Missing on Mobile

- [Custom fonts (`fontSrc` on `TextShape`, `Label`, `Button`, `Input` and `Dropdown`)](/docs/creator/sdk7/2d-ui/ui_text/#custom-fonts) — the mobile app ignores `fontSrc` and renders the fallback `font` — No ETA
- [`AvatarNametag` (scene-authored label above a player's nametag)](https://github.com/decentraland/godot-explorer/issues/2811) — No ETA
- [Tween sequences (`TweenSequence`)](/docs/creator/sdk7/3d-essentials/move-entities/#tween-sequences) — not implemented; sequenced tweens don't animate — No ETA
- [SDK7 UiBackground nine-slice tiles instead of stretching](https://github.com/decentraland/godot-explorer/issues/2060) — No ETA
- Draco mesh compression (`KHR_draco_mesh_compression`) — the mobile app has no runtime Draco decoder. Draco-compressed `.gltf`/`.glb` models load only from the scene's optimized assets, where the asset server has already decompressed them. When a scene has no optimized assets, as in local preview, they fail to load without an error. Export your models without Draco compression to be safe. An error dialog for this case is tracked in [#2432](https://github.com/decentraland/godot-explorer/issues/2432) — No ETA
- Smart Items — not officially supported on mobile

## Desktop Client Features Not in Mobile

- [Proximity Voice Chat](https://github.com/decentraland/godot-explorer/issues/888) — Q1 2027
- [Point-At In World](https://github.com/decentraland/godot-explorer/issues/1736) — No ETA
- [Nameplate Color Change](https://github.com/decentraland/godot-explorer/issues/1684) — No ETA
- [Communities](https://github.com/decentraland/godot-explorer/issues/656) — Q1 2027
- [Photo Gallery](https://github.com/decentraland/godot-explorer/issues/680) — December 2026
- [Community Streams](https://github.com/decentraland/godot-explorer/issues/676) — Q1 2027
- [Profile Badges](https://github.com/decentraland/godot-explorer/issues/678) — December 2026
- [Daily Quests](https://github.com/decentraland/godot-explorer/issues/682) — No ETA
- [Chat Reactions](https://github.com/decentraland/godot-explorer/issues/1824) — Q1 2027
- [Chat Auto-Translation](https://github.com/decentraland/godot-explorer/issues/2260) — Q1 2027
- [Chat: Direct Messages](https://github.com/decentraland/godot-explorer/issues/1120) — Q1 2027
- [DCL Cast Support on mobile](https://github.com/decentraland/godot-explorer/issues/1881) — No ETA
- [YouTube / Google Drive Video URLs unsupported](https://github.com/decentraland/godot-explorer/issues/2081) — Restricted in mobile for store compliance
- [Outfit Slots in Backpack](https://github.com/decentraland/godot-explorer/issues/1625) — December 2026
- [Gifting wearables to other players](https://github.com/decentraland/godot-explorer/issues/2957) — December 2026
- ["Session ended" notice when the same account signs in on another device](https://github.com/decentraland/godot-explorer/issues/2880) — No ETA
- [Discover categories (browse experiences by category)](https://github.com/decentraland/godot-explorer/issues/3037) — No ETA

## Cross-Platform Inconsistencies

- **Player movement and collisions vs Unity**
  - [Slopes, edges, walls, landing and moving platforms](https://github.com/decentraland/godot-explorer/issues/2852) and [glide behavior](https://github.com/decentraland/godot-explorer/issues/2854) — fix in review, No ETA
  - [Scene colliders thinner than 1 cm only collide from above, and invisible blockers and some SDK collision layers don't block the player](https://github.com/decentraland/godot-explorer/issues/2853) — No ETA
  - Spawn, landing and jump animations, movement blending ([#1553](https://github.com/decentraland/godot-explorer/issues/1553), [#1554](https://github.com/decentraland/godot-explorer/issues/1554), [#2855](https://github.com/decentraland/godot-explorer/issues/2855), [#2856](https://github.com/decentraland/godot-explorer/issues/2856)) — fix in review, No ETA
- [Some emotes from other clients don't play on mobile: scene-triggered emotes from Bevy Web players, and Unity players' emotes that arrive over LiveKit](https://github.com/decentraland/godot-explorer/issues/2986) — fix in review, No ETA
- [In a Creator Hub mobile preview, the mobile player's avatar is not visible to desktop testers](https://github.com/decentraland/godot-explorer/issues/2773) — fix in progress, No ETA
- [Avatar teeth render dark/gray instead of white](https://github.com/decentraland/godot-explorer/issues/1994) — No ETA

## Input/Platform Constraints

- **Touch-only input** — no mouse hover states, keyboard shortcuts, or right-click.
- **Limited gesture support** — pinch-to-zoom (switching between first- and third-person camera) was added in v1.14.0; no other gestures are currently planned.

## Report a Missing Feature

If you hit a limitation that is not listed here, please [report it](/docs/creator/sdk7/debugging/report-bug/) so we can document and prioritize it.
