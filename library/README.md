# `library/`

Shared code ([rules](../docs/adr/2026-10-19-organizing-library-services-folders.md)).
The one exception is `ui/apps/<app>/`, which holds UI used by a single app
([shared UI library ADR](../docs/adr/2026-10-30-shared-ui-library.md)).

| Path | What | Used by |
| --- | --- | --- |
| `dcl-auth/` | Hashes, signatures, auth chains, signed fetch, wallets | catalyst, scene CLI |
| `dcl-http/` | API errors, capped bodies, JSON-RPC, outbound client | catalyst, dcl-auth |
| `dcl-livekit-token/` | Offline LiveKit tokens | comms, SDK preview |
| `dcl-scene-authoring/` | Composites, CRDT encoding | scene CLI, scene build |
| `dcl-scene-build/` | SDK7 scene build, native and WASI | sites, scene CLI tests |
| `dcl-preview-tunnel{,-protocol}/` | Preview relay and wire types | scene CLI |
| `catalyst-api/` | TypeScript DTOs, OpenAPI documents | sites, ui3, catalyst |
| `protocol/` | The `.proto` contract, the npm package `@dcl/protocol` | Explorer (`dcl_component`), comms; SDK, social and the scene crates to follow |
| `ui/` | Shared UI foundations, components, app-specific UI (`apps/<app>/`) and Figma tooling ([standard](../docs/ui-standards.md#shared-ui-libraryui)); only the component template exists today | None yet; planned: web apps under `apps/`, the Explorer React HUD |
| `vendor/rusty_enet/` | Pinned ENet-CSharp transport source, MIT license and reviewed local patches | Catalyst Pulse, native Bevy Pulse client |
