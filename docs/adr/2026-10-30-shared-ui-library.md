# ADR: Shared UI library

- Status: Accepted
- Date: 2026-10-30

## Context

The web applications and the Explorer's React HUD need shared UI foundations
and components. Keeping their design values and reusable controls together
supports consistent use across deliverables.

## Decision

Place shared UI foundations, components and their Figma tooling in `library/ui`.
Place UI used by a single app in `library/ui/apps/<app>/`; the app keeps its
pages, routes and data wiring. The shared component styling standard is CSS
Modules. The Figma tooling is separate from browser imports.

### Layout

`library/ui` is used by several deliverables (the web apps and the Explorer
HUD), as the [layout ADR](2026-10-19-organizing-library-services-folders.md)
requires of `library/`. It is a TypeScript source library, not a Rust crate, so
the crate rules there (`dcl-*` name, `[workspace.dependencies]`) do not apply.

`library/ui/apps/<app>/` is an exception to that ADR's rule that `library/`
holds only code several deliverables use: by owner decision, UI used by a
single app lives there, beside the shared foundations and components it is
built from. The exception covers only `library/ui/apps/`; when a second
deliverable needs something from it, it moves to the shared `components/` or
`foundations/`.

## Consequences

Applications and the Explorer HUD will consume shared UI from the same library
after their integrations are reviewed and implemented. This change establishes
the standard and template; it does not migrate consumers. The
[Explorer HUD transition](../ui-standards.md#explorer-hud-transition) keeps the
current source guidance in effect until that integration is ready. Migration is
incremental, and replaced implementations are removed after consumers
move. The library's integration must account for independent builds,
asset resolution and artifact provenance. Existing behavior and applicable
surface themes must be preserved during migration.
