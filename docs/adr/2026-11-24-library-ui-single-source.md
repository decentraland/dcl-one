# ADR: library/ui is the single source of UI components

- Status: Proposed
- Date: 2026-11-24
- Amends: [Shared UI library](2026-10-30-shared-ui-library.md), [Shared UI import name](2026-11-09-shared-ui-import-name.md), [Shared Explorer HUD UI](2026-11-10-shared-explorer-hud-ui.md), [Shared UI imports through the package entry](2026-11-23-shared-ui-entry-imports.md)

## Context

`library/ui` publishes the shared components and foundations as `@dcl/one-ui`, and Sites consumes them. Other surfaces still own a component kit with the same purpose: the `apps/ui3` components (used by the web Shop and the `/social` page, both served by Sites on React 19) and the atoms they build from (also used by Creator Hub and the inspector), the Explorer HUD kit in `library/ui/apps/hud/design` (used by the HUD and the in-world Shop), the inspector's `components/ui`, and `library/ui/apps/sites/shared`. A button, a modal, a spinner or a set of tabs therefore exists in several implementations, each with its own tokens, and a design change has to be made once per kit.

Two of these surfaces cannot import the public components today. The Explorer HUD build maps only `@dcl/one-ui/foundations`, and its checked input graph refuses CSS Modules composition, sources outside its listed library folders and unmaterialized Git LFS assets, all of which the root entry reaches. Creator Hub and the inspector run React 18, and `@dcl/one-ui` declares React 19 as its peer.

## Decision

`library/ui` is the only source of UI components and design tokens for every web UI in this repository: Sites, the Shop, the Explorer HUD and its in-world Shop on both hosts, Creator Hub and the inspector.

- A component with a public equivalent in `library/ui/components` has no other implementation. A surface that needs a different look or behavior adds a variant to the shared component.
- A component that more than one surface needs and the library lacks is written in `library/ui/components` when its first consumer migrates.
- App-specific UI lives in `library/ui/apps/<app>/` and is built from the shared components and tokens.
- Every surface imports shared UI from the `@dcl/one-ui` entry and styles with the `--dcl-*` tokens.
- An app-owned kit whose consumers can already import the entry may only shrink: `apps/ui3/src/components` and `library/ui/apps/sites/shared` (the `KIT_PATHS` of `tooling/ui_standards.py`). `--kit-ceiling` fails when one grows above its recorded ceiling or above its size at the base revision, and when a recorded ceiling is above the base revision's. It counts files, so it does not detect one component replacing another: that stays a finding of the [review checklist](../ui-standards.md#review-checklist).
- The `apps/ui3` atoms (which the Creator Hub renderer and the inspector build from), the Explorer HUD kit (`library/ui/apps/hud/design`) and the inspector's `components/ui` keep receiving the components their surfaces need until those surfaces can import the entry. Each joins `KIT_PATHS` in the change that makes the entry importable there.
- `apps/ui3` is not the destination of any UI, including Creator Hub's and the inspector's.

Scene UI written with the SDK's `react-ecs` and pages that do not use React (`apps/dcl-scene-cli`, the HTML pages services render, `rendering/avatar-preview`) are outside this decision.

A migration covers one component and its consumers in one surface, records its adoption in the [adoption tracker](../ui-components.md) and is verified in each consuming surface ([UI standards](../ui-standards.md#migrating-existing-styles)).

## Consequences

- The Explorer HUD's foundations-only entry, its own design primitives and tokens, and the exemption of `library/ui/apps/hud` from the library's lint, formatting and import checks are the current state, not the target. The Explorer build resolves the `@dcl/one-ui` entry ([ADR](2026-11-24-explorer-hud-shared-entry.md)), so each of them ends as the HUD adopts the shared components and tokens, one bounded component at a time.
- A change to a component the HUD imports from the entry changes the HUD's build fingerprint and selects the Explorer's CI jobs.
- Shared components that position against the viewport (`Modal`, `Toast`, `Dropdown`) account for the HUD's runtime scale before the HUD adopts them.
- Creator Hub and the inspector adopt the shared components after they run a React version the package supports.
- The `/play` host and the Sites routes that import `apps/ui3` move to `library/ui` and to the app that owns the route, as [web application standards](../web-app-standards.md) require.
- Requested components and their implementation pull requests stay in [the shared UI backlog](https://github.com/decentraland/dcl-one/issues/644).
