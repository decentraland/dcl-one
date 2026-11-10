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
  Vitest configurations), so code one host leaves out never reaches its bundle.
- **No packages of its own.** `apps/hud` adds nothing to `library/ui/package.json`. Each host
  resolves React and the other packages to its own copy (`paths`, `resolve.dedupe`), at the versions
  the hosts already pin; `library/ui`'s own tests use the packages `library/ui` already pins.
- **The HUD's conventions.** The code keeps the compiler profile and style of `react-web`, which
  type-checks it, tests included. `library/ui`'s lint and formatting skip `apps/hud`. Its tests that
  need no host run in `library/ui`'s `hud` Vitest project against a web host; tests that drive a
  host's session stay in that host.
- **One copy of the bindings.** The TypeScript generated from the Explorer's Rust structs is tracked
  once, in `library/ui/apps/hud/engine/generated`, beside the wire protocol that uses it.
  `rendering/bevy-explorer/source/scripts/gen-ts-bindings.sh` writes it, and the Sites import checks
  the tracked copy against the build.
- **It moves in slices.** A slice holds only files whose import closure stays in
  `library/ui/apps/hud` or reaches the host through `@hud-host`. The rest of the HUD stays copied
  in both hosts until it moves.

## Consequences

Changing shared HUD code changes both hosts. `react-web` type-checks it, `library/ui` runs its
tests that need no host, each host's own tests exercise it through that host, and the HUD
artifact's inputs and CI selection include `library/ui/apps/hud/` (`HUD_SHARED` in
`tooling/play-artifact.py`). The Explorer client team owns `library/ui/apps/hud/`. Moving the rest
of the HUD needs the packages it imports (for example `@noble/*` for identity and
`@tanstack/react-query` for the Shop) to resolve in `library/ui`'s own tests, and the in-world
Shop's UI, which imports `apps/ui3/src/shop`, cannot move until those imports are in `library/`.
