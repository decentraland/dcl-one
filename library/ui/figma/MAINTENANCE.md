# Figma tooling maintenance

Maintenance notes for the Figma pipeline in `library/ui/figma` (capture, mapping rules, generation, diff, sync, verification). Consumers read `agent-kit/skills/implement-figma-design/SKILL.md`; the library overview is `library/ui/README.md`.

## Layout

| Path                                                | Role                                                                                                                                                                                     |
| --------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `library/ui/figma/`                                 | Reads Figma data (snapshots), maps it, generates code. CLI `dcl-figma` (`pnpm figma <command>`) and MCP server (`pnpm figma:mcp`, `figma_*` tools).                                      |
| `library/ui/figma/review/`                          | React previews and comparison controls. The local HTML comparison gallery is the primary visual review surface. Generated styling is changed through templates, never by editing output. |
| `library/ui/components/`, `library/ui/foundations/` | The shared library: hand-written components (Button, Modal), promoted previews (`components/<Name>/`), their `*.figma.ts` mappings and the foundations.                                  |
| `library/ui/figma/agent-kit/`                       | The consumer skill kit (`implement-figma-design`).                                                                                                                                       |

Figma file: PRUEBA-SKILL. Its key is private: set it in `FIGMA_FILE_KEY` (never in a file); mappings name it through the `<DCL_FIGMA_FILE>` placeholder. Page to category mapping lives in `library/ui/figma/figma.config.json`.

**Generated vs hand-written.** Everything under `figma/review/src/generated/` is overwritten by `sync` and carries a do-not-edit header. Hand-written Review code goes in `figma/review/src/components/` (runtime helpers) and `figma/review/src/review/`, and only imports from `generated/`.

## Three sources, never mixed up

| Source         | What it is                                               | Where                                                   | Authority for                                             |
| -------------- | -------------------------------------------------------- | ------------------------------------------------------- | --------------------------------------------------------- |
| Live Figma     | The file itself, read through the official Figma MCP     | Figma                                                   | Nothing is stored from it directly                        |
| Saved capture  | Snapshots of what the design looked like at capture time | `figma/figma-snapshot/`                                 | Design identity, variants, tokens, generated Review views |
| React contract | Which real component implements which Figma component    | `*.figma.ts` beside the component, owned by the library | What to import and which props to pass                    |

`sync` (generate/maintain) turns the capture into Review-only preview code in `src/generated/`. `figma_resolve_component` (resolve) reads the contract and never writes code. A generated preview is not an implementation: do not import it from an application.

## Contract (Code Connect templates)

The contract is the library's Code Connect template files, read statically (never executed) from `contract.mappingRoot` in `figma.config.json`, overridable with `--mapping-root`, `DCL_FIGMA_MAPPING_ROOT`, `--library-range`. Supported subset: header comments `url`, `source`, `component`, `contract`, `states`; `instance.getEnum/getBoolean/getString/findText`; `figma.tsx`/`code` example; `imports`; `id`. Anything else is reported as `unsupported-construct`.

- Provenance comes from the mapping header (Figma file and node) plus the nearest `package.json` of the library (name, version). Names alone never identify a component.
- Errors are coded and never guessed around: `mapping-missing`, `identity-ambiguous`, `identity-not-found`, `variant-unsupported`, `variant-not-in-figma`, `contract-incompatible` (with `details.reason`), `unsupported-construct`, `invalid-input`, `mapping-root-not-configured`. Warnings: `figma-drift`, `unmapped-figma-property`, `figma-identity-unverified`, `export-unverifiable`.
- `dcl-figma check-mappings` (and the test `src/contract/integrity.test.ts`) fails CI on any mapping error or Figma drift.
- Running the MCP from another project: server flags `--config`, `--mapping-root`, `--library-range`; without a config it runs resolve-only. See `library/ui/README.md`.

## Data sources: offline sync and guarded live capture

`dcl-figma sync` reads committed snapshots offline. The guarded `pull-live` command uses `FIGMA_TOKEN` to read live readiness and capture changed ready component pages through REST. The full sync and promotion workflows stay in figma-design-system and are not part of this repository. The older `--source rest` and `probe` path remains separate and unverified. The Variables REST API is Enterprise-only; the Plugin API through `use_figma` reads variables. `pull-live` reads Colors from the swatch texts of a live-ready Colors page, never from variables: each swatch's hex text must equal its fill, and a swatch where they differ fails the run with its node id. Gradient swatches are skipped; other ready Foundations pages are listed under `notCaptured` in the report. `pull-live --page <name>` refreshes one page only: an unknown page, a page without its sync control or a page that is not ready fails before anything is read or written, and every other page keeps its saved capture and readiness. A structured layer that paints an image fill gets its image downloaded once per Figma image hash and stored with its component as a data URI; a missing or non-raster image fails the capture. Every imported raster (capture artwork, image fills, icons and Review reference exports) is stored without non-rendering metadata: tEXt, iTXt, zTXt, eXIf and tIME are dropped and the rendering and colour chunks are kept byte for byte. `figma/test/image-metadata.test.ts` fails on any such chunk in a file under library/ui/figma (the frozen test capture included; the strip tests plant synthetic metadata at test time) or library/ui/components/assets, except the public files listed, each with a reason and an expiry, in `figma/image-metadata-allow.json`.

Unit tests read a frozen copy of the capture and of the Button and Modal mappings in `figma/test/fixtures/` (Git LFS for the capture), so a capture refresh cannot fail them; `check-mappings` is what verifies the live library against the live capture. Refresh the copy with `pnpm figma:freeze-snapshot` only when a test needs newer capture data, then run key-guard: the fixture config never carries the file key.

Before any Figma call: `create_session` with the file key, and load the `figma:figma-use` skill before `use_figma`. Capture scripts are **read-only**: never run a mutating `use_figma` script on this file.

## Refresh workflow

1. `create_session` (fileKey above), then read the page list: `get_metadata` without nodeId.
2. Tokens and icons: re-run the read-only extraction through `use_figma` (variables, paint and text styles, icon SVGs through `exportAsync`) and write the results to `figma-snapshot/`.
3. Components of a page: run `capture/extract-components.figma.js` through `use_figma` with `PAGE_IDS` set. Output is capped near 48 KB, so re-run with the returned `resume` values until it is `null`. Save each chunk to a file and ingest: `pnpm figma capture-ingest <chunk.json>...` (writes `figma-snapshot/components/<Page>.json`; it validates PNG assets and checksums, because chunks pass through the chat transcript).
   - **Transport parts:** a large capture returns `transport` with `checksum`, `offset`, `part` and `next`. Repeat the identical capture with `OUTPUT_START = transport.next` until it is null. Save every raw response. `capture-ingest` reconstructs the original JSON and rejects missing, overlapping or corrupted parts before writing. Large components retain their full tree; never truncate them or replace them with a screenshot to fit the tool response.
   - **Never retype a chunk.** Copy the raw `use_figma` result out of the agent session transcript with a script. In the 2026-10-27 recapture, 2 typos in retyped chunks hit an SVG/PNG and were caught by the asset checksums, 3 components differed from Figma only in plain JSON that no asset checksum covers (found by `VERIFY`), and 6 more retyped files were not byte-identical to the raw output.
   - **Verify against Figma:** `VERIFY = true` returns one checksum per component (FNV of `JSON.stringify(entry)`, the same `checksum` as for assets), small enough to run many pages in one call. Compare it with the same hash computed on the chunk files. Run it with the same `OTHERS_ROOT_ONLY` that produced the chunk.
   - **Ingest semantics:** chunks that cover a whole page (`start` 0 and every component of `total`) replace the page, so components deleted in Figma leave the snapshot (`removed` in the report). A partial set of chunks upserts by id. A renamed page is matched by id and its old file is removed.
   - **A set larger than one run** even with `OTHERS_ROOT_ONLY` (the 192-variant `Button`): set `START_COMPONENT` to it and `VARIANT_LIMIT`/`VARIANT_START` to capture its variants in parts; follow `resume.variant`. Ingest every part in the same `capture-ingest` call, because it joins them and refuses a set with missing variants.
4. Add the page to `figma.config.json` if new (`kind`, `category`).
5. `pnpm figma diff` shows what would change (exit 1 if not empty), then `pnpm figma sync`.
6. Run `sync` a second time: the diff must be empty (idempotency is tested).
7. Checks (format, lint, typecheck, tests, build, `diff` drift) run in GitHub Actions (`.github/workflows/ui-library.yml`); push and read the CI result instead of running the full suite locally. Run locally only what is needed to debug a failure.
8. Visual check: render the component in the local HTML comparison gallery and compare each variant/state with the actual Figma source, including image assets, typography, dimensions and backgrounds. Use Review for all preview checks. State exactly what differs; never say "matches" from compiling alone. Keep unreviewed components pending.

## Mapper rules (variants to props)

Figma variant names are inconsistent (`Property 1=Primary,Hover=No,Blocked=Yes`, `Terciary`, `Hidding`, `Variant2`). Rules the mapper applies:

1. A variant value is split into words. `Hover`/`Pressed`/`Blocked` become `:hover` / `:active` / prop `disabled`. `Selected` and `Loading` are real props. `Idle`/`Default`/`Normal`/`Unselected` are the base state.
2. `Variant2/3/4` are read as hover, selected, selected+hover. This is an inference; the manifest keeps a note. Check the screenshot before trusting it.
3. Per-component overrides are manual config in `figma.config.json`: `variantAliases`, `stateWords`, `optionAliases`, `propNames`, `skipOptions`. FriendCTA and Button-Multiple-Options needed aliases.
4. Layers that appear on hover are matched by name, not by index.
5. Instances resolve by `componentId`, never by name.
6. A color with no bound variable is searched by value in the palette; with no match it stays a literal hex.
7. `line-height` is emitted in px when the measured text height differs from the text style's.
8. Children of frames without auto-layout are positioned from captured x/y, taken from `absoluteBoundingBox` differences. A parent with absolutely positioned children becomes `position: relative`.
9. The capture drops hidden layers, retains effective instance children and exports vector artwork with checksums. Large component trees stay structured and travel in transport parts. Keep individual stroke widths, full-precision gradient transforms, text runs, clipping and alpha-mask assets.
10. Artwork is exported inline (SVG or PNG) with an FNV checksum and painted as `background`. Too large to inline means `heavy-artwork`.
11. Layers that only hover/pressed adds are grafted in hidden, and the state diff reveals them.
12. Use full variant trees for visual validation. `OTHERS_ROOT_ONLY` is an explicit reduced capture mode and cannot establish fidelity for nested state changes.
13. Components used on a page but defined elsewhere are captured by id into a synthetic `External` page (category Shared).
14. Per-variant icon aliases use the key `Set#Property=Value`.
15. A rotated `LINE` takes the size of its exported SVG. A non-HUG text taller than one line wraps.
16. Chunks that travel through the chat transcript must be validated with a checksum (PNG CRC, SVG shape) before ingest.

## needs-manual

A component is flagged when the mapper cannot reproduce it confidently. Reasons seen so far: `custom-glyph` (drawn shapes with no SVG), `unresolved-instance` (instance of a component on another page), `state-adds-layers` (hover adds layers, e.g. tooltips), `truncated-capture`, `slot-property`, `heavy-artwork` (too large to inline), `state-without-base-variant`, `variant-collision`. The reason is logged and shown in the Review component details. Report the count and reasons after each sync; do not hide them.

## Rules

- Follow CLAUDE.md's component architecture rules: custom browser primitives, no Base UI or other component kit, co-located CSS Modules instead of Emotion/styled-components, and TanStack only when shared state or data caching is needed. Apply changes to generated components through their templates.
- Everything in the repo is English: code, comments, tests, docs.
- Dependencies: exact versions (`--save-exact`), `@dcl/*` and `decentraland-*` with caret. Install with `pnpm install --frozen-lockfile` in `library/ui`.
- Never write a Figma token or any secret to a file or to chat. `.env.example` holds placeholders only.
- Never hand-edit `figma/review/src/generated/**` (CI `diff` is the enforcement).
- Keep explicit mappings and inference apart: a prop name read from a `*.figma.ts` is explicit; `Variant2/3/4` meaning is inference and must stay labelled.
- Preserve `needs-manual` flags; never clear one to make a count look better.
- Capture is read-only on Figma unless the user explicitly asks to edit the file.

## Known gaps (update after each phase)

- Recaptured 2026-10-27 (every component page plus the 4 Cards pages, category `Cards`): 172 components, 56 flagged `needs-manual`. Reason occurrences: unresolved-instance 33, truncated-capture 18, heavy-artwork 17, state-without-base-variant 7, variant-collision 5, slot-property 1, custom-glyph 0 (the newer extractor exports those vectors as SVG). A component can carry several.
- Unresolved instances point mostly at Assets pages (`ModalsImg`, `Wearables`, `AvatarSnapshot`) and at icons outside the icon set (`Icn/VerifiedOutline`, `Icn/Verified`, `ReminderIcn`). Capturing them into `External` or adding icon aliases is still open. Tokens and icons were not recaptured on 2026-10-27.
- The 192-variant `Button` (15721:13657) is captured in 5 variant parts and is `truncated` at depth 2.
- Verified visually so far: Colors, Icons, Button, FriendCTA, TextInput, Crosshair, Dropdown, Toast (before the recapture). Nothing from the 2026-10-27 recapture was compared against Figma screenshots yet: the new Bars `Nav*` set, Date pickers, Search results, InfoPanel, Cards pages and the 192-variant `Button` compile but are unverified.
- Known visual diffs: Toast ErrorCTA shows the instance default text instead of the overridden one; ErrorClose X is off-centre; Dropdown Flags renders as a placeholder; Dropdown `all-states` Opened view overlaps the next one.
- SVG assets from the first Phase 4 chunks (Progress, Spinner, Photos, Profile Picture, Tags, Toasts) have no per-asset checksum.
- Gradient angles are approximate (Flare 219 degrees, Light Rainbow 105 degrees).
- `map/avatar-pic` icon is skipped (embeds a PNG).
- Phase 6 (Assets/Backgrounds) is not captured.

## Preview output and local review

New generated previews use custom React/browser primitives and co-located CSS Modules through `figma-module-runtime`. Existing snapshots and legacy previews are not automatically promoted. Keep pilot capture and output paths explicit with `--config`; paths resolve relative to that configuration. Compare each source variant with its current render, including masks, mixed text styles, images and overflow. A forced state is a visual preview, not an implementation of keyboard, focus, click, validation or animation behavior. Real library components must implement those behaviors accessibly.
