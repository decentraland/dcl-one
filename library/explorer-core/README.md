# explorer-core

The Explorer page's core, shared by both HUD hosts (`rendering/bevy-explorer/source/react-web` and
Sites' `apps/ui3/src/play`) and by the Explorer's bridge scene: the page-engine protocol and the
bindings generated from the Explorer's Rust structs (`engine/`), the drivers' contract, sign-in,
shell messaging, analytics, the server reads written without React, and the host contract
(`hostContract.ts`, which each host's `@hud-host` module implements). The HUD UI,
`library/ui/apps/hud`, is built on it.

It imports no React and nothing of the HUD UI (`boundary.test.ts`). It has no `package.json`:
each host resolves its packages to its own copy. `react-web` type-checks it and runs its tests
(`npm test` there). Decision: [ADR](../../docs/adr/2026-11-10-shared-explorer-hud-ui.md).
