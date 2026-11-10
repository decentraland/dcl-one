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

Place the Explorer HUD the two hosts share in `library/ui/apps/hud`, the app-specific folder the
[shared UI library ADR](2026-10-30-shared-ui-library.md) gives a single app's UI. Both hosts
import it as `@hud/*`.

This is an exception to that ADR's rule that an app keeps its own data wiring. The HUD's two hosts
run one session against one engine, so its data wiring is shared like its UI and lives beside it:
the session (`features/session/useEngineSession.ts`), the page-engine protocol and bindings, sign-in
(`features/auth/sso.ts`) and the HUD's server reads (`features/*/…Api.ts`). Each host keeps only the
wiring that is its own: entry points, page shell, host module, engine drivers and engine host.

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
- **Hosts keep only their own code.** A file is shared when its import closure stays in
  `library/ui/apps/hud` or reaches the host through `@hud-host`. Each host keeps its entry points,
  shell, host module and the libraries behind it, the engine drivers and engine host, `App.tsx`,
  its global stylesheet and what only it has. Where the two copies had drifted, the shared one
  kept the fix or feature either host had.

## Consequences

Changing shared HUD code changes both hosts. `react-web` type-checks it and runs its tests,
`library/ui` runs the tests that need only its packages, and the HUD
artifact's inputs and CI selection include `library/ui/apps/hud/` (`HUD_SHARED` in
`tooling/play-artifact.py`). The Explorer client team owns `library/ui/apps/hud/`. Moving the rest
of the HUD keeps its tests that need other packages (for example `@noble/*` for identity) in
`apps/hud/test/hosted/`, which only `react-web` runs. The in-world Shop's UI imports
`apps/ui3/src/shop`, so it cannot move until those imports are in `library/`.
