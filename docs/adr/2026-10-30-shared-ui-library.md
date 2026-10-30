# ADR: Shared UI library

- Status: Proposed
- Date: 2026-10-30

## Context

The web applications and the Explorer's React HUD need shared UI foundations
and components. Keeping their design values and reusable controls together
supports consistent use across deliverables.

## Decision

Place shared UI foundations, components and their Figma tooling in `library/ui`.
Keep feature-specific UI with its feature. The shared component styling
standard is CSS Modules. The Figma tooling is separate from browser imports.

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
