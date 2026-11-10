# ADR: The shared Explorer HUD: `library/explorer-core` and `library/ui/apps/hud`

- Status: Accepted
- Date: 2026-11-10

## Context

The Explorer's React DOM HUD has two hosts. `rendering/bevy-explorer/source/react-web` builds it
for the desktop app and the CDN web Explorer. `apps/ui3/src/play` builds it for Sites under
`/play`. Each host held its own copy of the HUD, nearly identical: the hosts differ in their entry
points, how they reach services, sign-in and a few presentation details, such as the native HUD
routing profile images through its own proxy.

Part of that code is not UI. The page-engine protocol and the bindings generated from the
Explorer's Rust structs are a contract between the engine, the bridge scene and both hosts. The
drivers, sign-in, shell messaging, analytics and server reads are plain TypeScript.

## Decision

Share the HUD in two layers. Dependencies point one way: each host depends on both layers, the UI
on the core, and the core on neither.

- **`library/explorer-core`, imported as `@explorer-core/*`: the Explorer page's core.** The
  page-engine protocol and generated bindings (`engine/`), the driver contract and drivers, sign-in,
  shell messaging, analytics, the server reads written without React, and the host contract. It
  imports no React and nothing of the HUD UI; `boundary.test.ts` enforces it. The bridge scene and
  the bindings generator use it directly.
- **`library/ui/apps/hud`, imported as `@hud/*`: the HUD UI.** Components, styles, assets and the
  React state around the core: the session (`features/session/useEngineSession.ts`), its stores,
  and the modules that mix a React hook with a server read. It is the app-specific UI the
  [shared UI library ADR](2026-10-30-shared-ui-library.md) places in `library/ui/apps/<app>/`.

The rules both layers follow:

- **One way down to the hosts.** Shared code imports only the two layers and the packages the hosts
  already pin. A behavior the hosts implement differently comes from the module `@hud-host`, which
  each host maps to its own `host.ts` implementing `library/explorer-core/hostContract.ts`. The
  mapping is a build-time alias (`paths` in each host's `tsconfig.json`, `resolve.alias` in its
  Vite and Vitest configurations), so code one host leaves out never reaches its bundle. A flow only
  one host has comes in the same way: react-web's benchmark autopilot is its `unattendedRun`.
- **The host module stays below the HUD.** A host's `host.ts` and what it imports never import
  shared code that reads `@hud-host`, so loading the host module does not load the HUD.
- **No packages of their own.** Neither layer adds to a `package.json`. Each host resolves React
  and the other packages to its own copy (`paths`, `resolve.dedupe`), at the versions it pins.
- **The HUD's conventions.** Both layers keep the compiler profile and style of `react-web`, which
  type-checks them and runs their tests. `library/ui`'s lint and formatting skip `apps/hud`; its
  `hud` Vitest project also runs the UI tests that need only `library/ui`'s packages (all but
  `apps/hud/test/hosted/`). Tests of a host's own files stay in that host.
- **One copy of the bindings.** The TypeScript generated from the Explorer's Rust structs is tracked
  once, in `library/explorer-core/engine/generated`.
  `rendering/bevy-explorer/source/scripts/gen-ts-bindings.sh` writes it, and the Sites import checks
  the tracked copy against the build.
- **Hosts keep only their own code.** Each host keeps its entry points, shell, host module and the
  libraries behind it, the engine drivers and engine host, `App.tsx`, its global stylesheet and what
  only it has. Where the two copies had drifted, the shared one kept the fix or feature either host
  had.

### Layout

`library/explorer-core` is used by the Explorer's own build (react-web and its bridge scene) and
by Sites' `/play` host, as the [layout ADR](2026-10-19-organizing-library-services-folders.md)
requires of `library/`. It is a TypeScript source library, like `library/shop`, so that ADR's crate
rules do not apply.

## Consequences

Changing shared code changes both hosts. `react-web` type-checks both layers and runs their tests;
the HUD artifact's inputs and CI selection include both (`HUD_SHARED` in `tooling/play-artifact.py`).
The Explorer client team owns both folders. A module that holds a React hook and a server read stays
in the UI layer until the two are split; the in-world Shop's UI imports `apps/ui3/src/shop`, so it
stays in `/play` until those imports are in `library/`.
