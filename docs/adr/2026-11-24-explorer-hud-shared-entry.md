# ADR: The Explorer HUD imports the shared UI entry

- Status: Proposed
- Date: 2026-11-24
- Amends: [Shared UI library](2026-10-30-shared-ui-library.md), [Shared UI import name](2026-11-09-shared-ui-import-name.md), [Shared UI imports through the package entry](2026-11-23-shared-ui-entry-imports.md), [library/ui is the single source of UI components](2026-11-24-library-ui-single-source.md)

## Context

The Explorer HUD (`library/ui/apps/hud`, built by `rendering/bevy-explorer/source/react-web` and by the Sites `/play` host) kept its own copies of the shared components because its build could not import `@dcl/one-ui`: `react-web` mapped only the foundations entry, and the HUD's checked input graph refused CSS Modules composition and any source outside four library folders. The shared components compose their text styles from `foundations/typography/text-styles.module.css`, and the entry reaches `library/web-core` through the Sites UI it exports.

## Decision

`react-web` maps `@dcl/one-ui` to `library/ui/index.ts`, and the HUD imports shared components from that entry like every other surface. The first one is the Lobby's loading placeholder, the `overlay` tone of `Skeleton`.

The checked input graph stays the source of the HUD's source selection, fingerprint, dev access, native staleness and Nix fileset:

- The generator resolves each `composes ... from` target itself, relative to the stylesheet or through the build's aliases, records it as a `composes` edge and walks the target as a stylesheet too. A target that does not resolve fails generation. Vite's pipeline still does not report these reads; the contract no longer depends on it.
- `library/web-core` is a source domain of the graph, in the generator, `tooling/hud_shared_inputs.py` and `infra/packages/hud-shared-inputs.nix`.
- The graph records the Git LFS images and videos the entry reaches. Every reader of the graph from a working tree refuses an unmaterialized pointer, so the CI jobs that build, package or verify the HUD fetch `library/ui/assets` and `library/ui/apps/sites/assets` with `git lfs pull --include`, not the repository's whole LFS content, and a local clone needs the same two folders pulled before `pnpm dev`, the tests or a native build. A reader at a Git revision accepts the pointers.

## Consequences

- Importing the entry adds the whole shared library to the graph, whatever the HUD renders. The graph holds 1642 files: among them the shared components, 342 files of the Sites UI the entry exports, six `library/web-core` modules and 63 Git LFS assets. A component the HUD does not render is compiled and fingerprinted, and its code and styles are left out of the bundle. Vite still emits the images and videos such a component imports, so the HUD build removes the shared library's emitted media that no chunk, stylesheet or page names (`dropUnusedSharedAssets` in `react-web/vite.config.ts`): with the `Skeleton` import alone the web build is 10 MB and 147 files, and 30 MB and 217 files without that step.
- A change to any file the entry reaches, including a Sites view, changes the HUD's fingerprint and selects the Explorer's CI jobs.
- A HUD copy of a shared component is replaced by the shared one in a change that covers that component and its HUD consumers; `react-web`'s agent instructions say to import the shared component when one exists.
- The HUD draws its notices, selects and dialog cards with `Toast`, the HUD kinds of `Dropdown` and `ModalCard`. None of them is portaled or placed against the viewport: the HUD's own containers place them, and the HUD `Dropdown` only compares on-screen boxes to choose the side it opens on. The shared `Modal` and the default `Dropdown` position against the viewport and need to account for the HUD's runtime scale before the HUD adopts them.
