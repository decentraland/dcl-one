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

### Foundation installation

The shared `figma install` command owns installation of public UI targets. Its foundations target regenerates colors, icons, typography, layout and spacing from the verified capture pinned in `library/ui/upstream.lock.json`. Live reads cannot write public output. Installation stages the complete target, refuses dirty or concurrently changed output and restores the previous directory when publication fails. Maintained CSS entrypoints, declarations and licensed Inter font assets are preserved. The public lock table records file digests and origins, and font validation compares materialized bytes with Git LFS object identities and their pinned digests. A port verification record binds the source lock and tooling tree to a clean installation that reproduces every tracked foundation file. Browser entrypoints remain independent of installation code.

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

### Component installation and integrity

The components target uses the same verified-cache source and atomic install transaction as foundations. A pure producer receives source data, configuration, a names-only visibility registry and the locked runtime; it cannot acquire data or publish files. The transaction owns and rechecks the complete input read-set. Public names receive mappings; internal names receive neither mappings nor package exports. Installation records source, configuration, dependency and complete generator-module identities alongside emitted output digests in `components/generation.json`. The common output inspector verifies the index and materialized bytes for both targets; the component policy derives origin classes from the emitted plan and fixed runtime inputs. Components never admit LFS pointers. The checked record establishes offline provenance consistency, not an authenticated claim of live Figma parity. By owner decision, NavBar is exposed as a public bridge for the Sites home, legal and blog pages with captured signed-in mock balances, notification counts and a placeholder avatar, empty mobile variants 7 and 8, pending semantic data and avatar contracts, and a 60px desktop and 48px mobile logo pending alignment to the 40px design.

### HUD shared-source contract

Foundation-only consumers use the public `@dcl/one-ui/foundations` entry; the root package continues to export public components. The Explorer HUD computes its shared-library closure with its actual native TypeScript compiler and Vite web/native graphs, including CSS imports and assets through Vite's processor. A deterministic tracked manifest binds owned HUD source roots, resolver and font identity controls, reached shared files and their graph edges. The changed-file selector, artifact fingerprint, Vite access list, native stale check and pure Nix source contract consume the same validated data. Stale or unresolved head inputs are refused; historical revisions without their own manifest cannot reuse an artifact and select the web build conservatively. The manifest excludes installed npm dependency contents and generated engine bindings, whose locked package and engine producer contracts remain separate. Inter font substitution remains controlled by the installed locked package and the shared font metadata. This import contract does not change the Sites HUD adaptation review scope.

The HUD graph refuses CSS Modules composition (`composes` and `compose-with`) because Vite's current PostCSS Modules pipeline does not report those file dependencies. Supporting composition requires upstream dependency reporting; this contract does not attest to arbitrary CSS processor reads. Inline PostCSS plugins remain part of the actual pipeline; automatic or external PostCSS configuration is refused instead of silently replacing it. Filtered source archives without Git validate the checked manifest against regular physical files and the complete HUD root inventory before use.
