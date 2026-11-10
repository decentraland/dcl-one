# Figma tooling maintenance

Maintenance notes for the Figma pipeline in `library/ui/figma` (mapping rules, generation, diff, sync, verification). The capture and its visual review live in figma-design-system. Consumers read `agent-kit/skills/implement-figma-design/SKILL.md`; the library overview is `library/ui/README.md`.

## Layout

| Path                                                | Role                                                                                                                                                                                        |
| --------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `library/ui/figma/`                                 | Reads Figma data (snapshots), maps it, generates code. CLI `dcl-figma` (`pnpm figma <command>`) and MCP server (`pnpm figma:mcp`, `figma_*` tools).                                         |
| `library/ui/figma/review/`                          | A gallery of the generated components, rendered live. It publishes no Figma imagery and does no visual comparison. Generated styling is changed through templates, never by editing output. |
| `library/ui/components/`, `library/ui/foundations/` | The shared library: hand-written components (Button, Modal), promoted previews (`components/<Name>/`), their `*.figma.ts` mappings and the foundations.                                     |
| `library/ui/figma/agent-kit/`                       | The consumer skill kit (`implement-figma-design`).                                                                                                                                          |

Figma file: PRUEBA-SKILL. Its key is private and never in a file; set `FIGMA_FILE_KEY` only to run the key guard's `key` check; mappings name it through the `<DCL_FIGMA_FILE>` placeholder. Page to category mapping lives in `library/ui/figma/figma.config.json`.

**Generated vs hand-written.** Everything under `figma/review/src/generated/` is overwritten by `sync` and carries a do-not-edit header. Hand-written Review code goes in `figma/review/src/components/` (runtime helpers) and `figma/review/src/review/`, and only imports from `generated/`.

## Three sources, never mixed up

| Source         | What it is                                               | Where                                                                                                                                      | Authority for                                             |
| -------------- | -------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------- |
| Live Figma     | The file itself                                          | Read by figma-design-system only, never by this tree                                                                                       | Nothing here is read or stored from it directly           |
| Saved capture  | Snapshots of what the design looked like at capture time | figma-design-system, pinned in `upstream.lock.json`; `pnpm figma fetch-source` materializes it into the gitignored `figma/figma-snapshot/` | Design identity, variants, tokens, generated Review views |
| React contract | Which real component implements which Figma component    | `*.figma.ts` beside the component, owned by the library                                                                                    | What to import and which props to pass                    |

`sync` (generate/maintain) turns the capture into Review-only preview code in `src/generated/`. `figma_resolve_component` (resolve) reads the contract and never writes code. A generated preview is not an implementation: do not import it from an application.

## Contract (Code Connect templates)

The contract is the library's Code Connect template files, read statically (never executed) from `contract.mappingRoot` in `figma.config.json`, overridable with `--mapping-root`, `DCL_FIGMA_MAPPING_ROOT`, `--library-range`. Supported subset: header comments `url`, `source`, `component`, `contract`, `states`; `instance.getEnum/getBoolean/getString/findText`; `figma.tsx`/`code` example; `imports`; `id`. Anything else is reported as `unsupported-construct`.

- Provenance comes from the mapping header (Figma file and node) plus the nearest `package.json` of the library (name, version). Names alone never identify a component.
- Errors are coded and never guessed around: `mapping-missing`, `identity-ambiguous`, `identity-not-found`, `variant-unsupported`, `variant-not-in-figma`, `contract-incompatible` (with `details.reason`), `unsupported-construct`, `invalid-input`, `mapping-root-not-configured`. Warnings: `figma-drift`, `unmapped-figma-property`, `figma-identity-unverified`, `export-unverifiable`.
- `dcl-figma check-mappings` (and the test `src/contract/integrity.test.ts`) fails CI on any mapping error or Figma drift.
- Running the MCP from another project: server flags `--config`, `--mapping-root`, `--library-range`; without a config it runs resolve-only. See `library/ui/README.md`.

## Data source: the fetched capture only

`dcl-figma sync`, `diff`, `check-mappings`, `review-manifest` and the MCP server read the capture offline, from the cache `pnpm figma fetch-source` builds from the commit of figma-design-system that `upstream.lock.json` pins (fail-closed: a missing, stale or edited cache stops every reader). Live Figma reads are for local inspection only (`diff --source rest`, the MCP read tools with `source: "rest"`, `figma_screenshot`, `probe`): they never write tracked files, `sync` refuses them, and nothing from them is stored in the tree. Capturing pages and ingesting chunks happen in figma-design-system. Every raster in the capture (artwork, image fills and icons) is stored without non-rendering metadata: tEXt, iTXt, zTXt, eXIf and tIME are dropped and the rendering and colour chunks are kept byte for byte. `figma/test/image-metadata.test.ts` fails on any such chunk in a file under library/ui/figma (the synthetic test capture included; the strip tests plant synthetic metadata at test time) or library/ui/components/assets, except the public files listed, each with a reason and an expiry, in `figma/image-metadata-allow.json`.

Unit tests read a hand-written synthetic capture in `figma/test/fixtures/synthetic-snapshot/` (plain git, fake ids and names, described in its README), never the real one, so they run in public CI and a capture refresh cannot fail them. The checks against the real capture (`diff`, `check-mappings`, `review-manifest`, the NavBar bounds) run locally after `pnpm figma fetch-source`, and a change to a port input records their results in `library/ui/port-verification.json` (`port-verification.mjs`; edits to existing tests, fixtures and Markdown are not port input changes).

## Refresh workflow

The capture is refreshed in figma-design-system, which owns it and documents how a capture is taken. This tree then ports the new commit:

1. Bump the pin in `upstream.lock.json`, then `pnpm figma fetch-source --seal`.
2. `pnpm figma diff` shows what would change (exit 1 if not empty), then `pnpm figma sync`.
3. Run `sync` a second time: the diff must be empty (idempotency is tested).
4. `node figma/lock-digests.mjs --write`, then run the real-capture checks and commit their record (`port-verification.mjs`). A deviation reason goes in `deviationCodes` under a new stable id; `--write` keeps the codes sorted.
5. Checks (format, lint, typecheck, tests, build, lock digests, key guard) run in GitHub Actions (`.github/workflows/ui-library.yml`); push and read the CI result instead of running the full suite locally. Run locally only what is needed to debug a failure.
6. `pnpm review:build` and `pnpm review:check-build` prove the gallery builds, navigates and renders. They do not prove visual parity: comparing a variant with Figma happens in figma-design-system. Never say a component "matches" Figma from this tree.

## Mapper rules (variants to props)

Figma variant names are inconsistent (`Property 1=Primary,Hover=No,Blocked=Yes`, `Terciary`, `Hidding`, `Variant2`). Rules the mapper applies:

1. A variant value is split into words. `Hover`/`Pressed`/`Blocked` become `:hover` / `:active` / prop `disabled`. `Selected` and `Loading` are real props. `Idle`/`Default`/`Normal`/`Unselected` are the base state.
2. `Variant2/3/4` are read as hover, selected, selected+hover. This is an inference; the manifest keeps a note. Check it against Figma in figma-design-system before trusting it.
3. Per-component overrides are manual config in `figma.config.json`: `variantAliases`, `stateWords`, `optionAliases`, `propNames`, `skipOptions`. FriendCTA and Button-Multiple-Options needed aliases.
4. Layers that appear on hover are matched by name, not by index.
5. Instances resolve by `componentId`, never by name.
6. A color with no bound variable is searched by value in the palette; with no match it stays a literal hex.
7. `line-height` is emitted in px when the measured text height differs from the text style's.
8. Children of frames without auto-layout are positioned from captured x/y, taken from `absoluteBoundingBox` differences. A parent with absolutely positioned children becomes `position: relative`.
9. The capture drops hidden layers, retains effective instance children and exports vector artwork with checksums. Large component trees stay structured and travel in transport parts. Keep individual stroke widths, full-precision gradient transforms, text runs, clipping and alpha-mask assets.
10. Artwork is exported inline (SVG or PNG) with an FNV checksum and painted as `background`. Too large to inline means `heavy-artwork`.
11. Layers that only hover/pressed adds are grafted in hidden, and the state diff reveals them.
12. Use full variant trees. `OTHERS_ROOT_ONLY` is an explicit reduced capture mode and misses nested state changes.
13. Components used on a page but defined elsewhere are captured by id into a synthetic `External` page (category Shared).
14. Per-variant icon aliases use the key `Set#Property=Value`.
15. A rotated `LINE` takes the size of its exported SVG. A non-HUG text taller than one line wraps.

## needs-manual

A component is flagged when the mapper cannot reproduce it confidently. Reasons seen so far: `custom-glyph` (drawn shapes with no SVG), `unresolved-instance` (instance of a component on another page), `state-adds-layers` (hover adds layers, e.g. tooltips), `truncated-capture`, `slot-property`, `heavy-artwork` (too large to inline), `state-without-base-variant`, `variant-collision`. The reason is logged and shown in the Review component details. Report the count and reasons after each sync; do not hide them.

## Rules

- Follow CLAUDE.md's component architecture rules: custom browser primitives, no Base UI or other component kit, co-located CSS Modules instead of Emotion/styled-components, and TanStack only when shared state or data caching is needed. Apply changes to generated components through their templates.
- Everything in the repo is English: code, comments, tests, docs.
- Dependencies: exact versions (`--save-exact`), `@dcl/*` and `decentraland-*` with caret. Install with `pnpm install --frozen-lockfile` in `library/ui`.
- Never write a Figma token or any secret to a file or to chat: `FIGMA_TOKEN` and `FIGMA_FILE_KEY` are read from the environment only.
- Never hand-edit `figma/review/src/generated/**` (CI `diff` is the enforcement).
- Keep explicit mappings and inference apart: a prop name read from a `*.figma.ts` is explicit; `Variant2/3/4` meaning is inference and must stay labelled.
- Preserve `needs-manual` flags; never clear one to make a count look better.
- Capture is read-only on Figma unless the user explicitly asks to edit the file.

## Known gaps (update after each phase)

- Recaptured 2026-10-27 (every component page plus the 4 Cards pages, category `Cards`): 172 components, 56 flagged `needs-manual`. Reason occurrences: unresolved-instance 33, truncated-capture 18, heavy-artwork 17, state-without-base-variant 7, variant-collision 5, slot-property 1, custom-glyph 0 (the newer extractor exports those vectors as SVG). A component can carry several.
- Unresolved instances point mostly at Assets pages (`ModalsImg`, `Wearables`, `AvatarSnapshot`) and at icons outside the icon set (`Icn/VerifiedOutline`, `Icn/Verified`, `ReminderIcn`). Capturing them into `External` or adding icon aliases is still open. Tokens and icons were not recaptured on 2026-10-27.
- The 192-variant `Button` (15721:13657) is captured in 5 variant parts and is `truncated` at depth 2.
- SVG assets from the first Phase 4 chunks (Progress, Spinner, Photos, Profile Picture, Tags, Toasts) have no per-asset checksum.
- Gradient angles are approximate (Flare 219 degrees, Light Rainbow 105 degrees).
- `map/avatar-pic` icon is skipped (embeds a PNG).
- Phase 6 (Assets/Backgrounds) is not captured.

## Preview output and local review

New generated previews use custom React/browser primitives and co-located CSS Modules through `figma-module-runtime`. Existing snapshots and legacy previews are not automatically promoted. Keep pilot capture and output paths explicit with `--config`; paths resolve relative to that configuration. A forced state is a visual preview, not an implementation of keyboard, focus, click, validation or animation behavior. Real library components must implement those behaviors accessibly.
