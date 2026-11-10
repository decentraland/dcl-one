# ADR: The Explorer HUD UI in `library/ui/apps/hud`

- Status: Accepted
- Date: 2026-11-10

## Context

The Explorer's React DOM HUD has two hosts. `rendering/bevy-explorer/source/react-web` builds it
for the desktop app and the CDN web Explorer. `apps/ui3/src/play` builds it for Sites under
`/play`. HUD code that only one host holds has to be written in both, and the two copies are
nearly identical: the hosts differ in their entry points, how they reach services, sign-in and a
few presentation details, such as the native HUD routing profile images through its own proxy.

## Decision

Place the HUD UI the two hosts share in `library/ui/apps/hud`, as the
[shared UI library ADR](2026-10-30-shared-ui-library.md) places app-specific UI. Both hosts
import it as `@hud/*`.

- **No host imports.** Shared HUD code imports only itself and the packages the hosts already
  pin. A behavior the hosts implement differently comes from the module `@hud-host`, which each
  host maps to its own `host.ts` implementing `library/ui/apps/hud/hostContract.ts`. The mapping
  is a build-time alias (`paths` in each host's `tsconfig.json`, `resolve.alias` in its Vite and
  Vitest configurations), so code one host leaves out never reaches its bundle. A flow only one
  host has comes in the same way: react-web's benchmark autopilot is its `unattendedRun`, and the
  shared session knows only that a host may have an unattended run.
- **The host module stays below the HUD.** A host's `host.ts` and what it imports never import
  shared HUD code that reads `@hud-host`, so loading the host module does not load the HUD.
- **No packages of its own.** `apps/hud` adds nothing to `library/ui/package.json`. Each host
  resolves React and the other packages to its own copy (`paths`, `resolve.dedupe`), at the versions
  the hosts already pin; `library/ui`'s own tests use the packages `library/ui` already pins.
- **The HUD's conventions.** The code keeps the compiler profile and style of `react-web`, which
  type-checks it, tests included. `library/ui`'s lint and formatting skip `apps/hud`. Its tests live
  beside it. `react-web`'s Vitest runs them all, with its own packages; `library/ui`'s `hud` project
  runs those that need no package only the hosts install. Tests of a host's own files stay in that
  host.
- **One copy of the bindings.** The TypeScript generated from the Explorer's Rust structs is tracked
  once, in `library/ui/apps/hud/engine/generated`, beside the wire protocol that uses it.
  `rendering/bevy-explorer/source/scripts/gen-ts-bindings.sh` writes it, and the Sites import checks
  the tracked copy against the build.
- **It moves in slices.** A slice holds only files whose import closure stays in
  `library/ui/apps/hud` or reaches the host through `@hud-host`. The rest of the HUD stays copied
  in both hosts until it moves.

## Consequences

Changing shared HUD code changes both hosts. `react-web` type-checks it and runs its tests,
`library/ui` runs the tests that need only its packages, and the HUD
artifact's inputs and CI selection include `library/ui/apps/hud/` (`HUD_SHARED` in
`tooling/play-artifact.py`). The Explorer client team owns `library/ui/apps/hud/`. Moving the rest
of the HUD keeps its tests that need other packages (for example `@noble/*` for identity) in
`apps/hud/test/hosted/`, which only `react-web` runs. The in-world Shop's UI imports
`apps/ui3/src/shop`, so it cannot move until those imports are in `library/`.
