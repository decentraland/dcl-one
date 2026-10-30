> Features available on the Decentraland desktop client that are not yet supported in the mobile app, and known cross-platform inconsistencies.

# Missing Features


> [!NOTE]
> This page tracks the feature gap between the Decentraland desktop (Unity) client and the mobile app. It is sourced from the [godot-explorer feature parity tracker](https://github.com/decentraland/godot-explorer/issues/2402) and updated regularly. ETAs are estimates and subject to change.
> 
> Last reviewed: **August 2026**.


## SDK Features Missing on Mobile

- [Scene Dynamic Lights (`LightSource`)](https://github.com/decentraland/godot-explorer/issues/616) — implemented in [PR #2633](https://github.com/decentraland/godot-explorer/pull/2633), ships in **v1.13.0 (September 2026)**
- [Avatar Masks (upper-body-only emotes)](https://github.com/decentraland/godot-explorer/issues/2621) — implemented in [PR #2690](https://github.com/decentraland/godot-explorer/pull/2690), ships in **v1.13.0 (September 2026)**
- [SDK7 UiBackground nine-slice tiles instead of stretching](https://github.com/decentraland/godot-explorer/issues/2060) — No ETA
- [SDK audio parity audit (volume curves and spatial attenuation vs Unity)](https://github.com/decentraland/godot-explorer/issues/861) — No ETA
- [AudioEvent component (`PBAudioEvent`)](https://github.com/decentraland/godot-explorer/issues/2673) — No ETA
- [Audio Analysis component](https://github.com/decentraland/godot-explorer/issues/1184) — No ETA
- Smart Items — not officially supported on mobile

## Desktop Client Features Not in Mobile

- [Proximity Voice Chat](https://github.com/decentraland/godot-explorer/issues/888) — No ETA
- [Point-At In World](https://github.com/decentraland/godot-explorer/issues/1736) — No ETA
- [Nameplate Color Change](https://github.com/decentraland/godot-explorer/issues/1684) — No ETA
- [Communities](https://github.com/decentraland/godot-explorer/issues/656) — No ETA
- [Photo Gallery](https://github.com/decentraland/godot-explorer/issues/680) — No ETA
- [Community Streams](https://github.com/decentraland/godot-explorer/issues/676) — No ETA
- [Profile Badges](https://github.com/decentraland/godot-explorer/issues/678) — No ETA
- [Daily Quests](https://github.com/decentraland/godot-explorer/issues/682) — No ETA
- Marketplace Credits — No ETA
- [Chat Reactions](https://github.com/decentraland/godot-explorer/issues/1824) — No ETA
- [Chat Auto-Translation](https://github.com/decentraland/godot-explorer/issues/2260) — No ETA
- [Chat: Direct Messages](https://github.com/decentraland/godot-explorer/issues/1120) — No ETA
- [DCL Cast Support on mobile](https://github.com/decentraland/godot-explorer/issues/1881) — No ETA
- [YouTube / Google Drive Video URLs unsupported](https://github.com/decentraland/godot-explorer/issues/2081) — Restricted in mobile for store compliance
- [Outfit Slots in Backpack](https://github.com/decentraland/godot-explorer/issues/1625) — No ETA

## Cross-Platform Inconsistencies

- [Colliders Shape Consistency Review vs Unity](https://github.com/decentraland/godot-explorer/issues/905) — September 2026
- [Avatar teeth render dark/gray instead of white](https://github.com/decentraland/godot-explorer/issues/1994) — No ETA

## Input/Platform Constraints

- **Touch-only input** — no mouse hover states, keyboard shortcuts, or right-click.
- **No gesture support** — not currently planned.

## Report a Missing Feature

If you hit a limitation that is not listed here, please [report it](/docs/creator/sdk7/debugging/report-bug/) so we can document and prioritize it.
