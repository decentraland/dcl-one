# ADR: The Sites UI has its own package entry

- Status: Proposed
- Date: 2026-11-24
- Amends: [Shared UI imports through the package entry](2026-11-23-shared-ui-entry-imports.md)

## Context

`library/ui/index.ts` re-exported the app-scoped Sites UI (`library/ui/apps/sites/index.ts`), so every consumer of one shared component reached all of it. Importing `Skeleton` from `@dcl/one-ui` in the Explorer HUD raised the HUD's checked input graph from 1179 to 1635 files: 340 under `library/ui/apps/sites`, 59 of the 61 Git LFS assets and six `library/web-core` modules. The HUD build does not fetch Git LFS, and any change to a Sites view would change the HUD's fingerprint and select the Explorer's CI jobs. The UI gallery already replaced the re-export with an empty module in its build for the same reason.

## Decision

`@dcl/one-ui` exports the shared foundations and components only. The app-scoped Sites UI is imported from `@dcl/one-ui/apps/sites`, which `library/ui/package.json` declares and which resolves to `library/ui/apps/sites/index.ts`; a path below it is still refused. The naming rules of the Sites entry stay: a name that collides with a shared export keeps its `Sites` prefix, so a file can import from both entries.

## Consequences

- The same `Skeleton` import from `@dcl/one-ui` now adds 110 files to the HUD's input graph, all under `library/ui`: the shared components, and the two Git LFS images of `AnimatedBackground`. It reaches no `library/web-core` module.
- `tooling/ui_standards.py --shared-imports` accepts `@dcl/one-ui/apps/sites` and fails on any path below it.
- Sites and `apps/ui3` already map `@dcl/one-ui/*` to `library/ui/*`, so the entry needs no new alias.
- The UI gallery compiles the shared entry as it is; its build no longer replaces the Sites re-export.
- A test that replaces a Sites view mocks `@dcl/one-ui/apps/sites`.
