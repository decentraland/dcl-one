# @dcl/one-ui

Shared UI foundations and components for dcl-one, and the Node-only Figma tooling that produces them.
The standards they follow are in [`docs/ui-standards.md`](../../docs/ui-standards.md).

| Path                   | What it is                                                                                                                                                                                                                                                                   |
| ---------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `figma/`               | Node-only: CLI and MCP server (`dcl-figma`), Review gallery (`review/`), consumer skill kit (`agent-kit/`), `key-guard.mjs`, `lock-digests.mjs`, `port-verification.mjs`. Never imported by browser code. Maintenance notes: [`figma/MAINTENANCE.md`](figma/MAINTENANCE.md). |
| `templates/component/` | Starting point for a new shared component.                                                                                                                                                                                                                                   |

## Shared foundations

The public `@dcl/one-ui` entry exports colors, icons, typography, responsive layouts and spacing, including semantic CSS references through `theme`. Load `@dcl/one-ui/foundations.css` once from the consumer's global stylesheet; it provides the shared CSS variables and bundled Inter fonts. Import typography classes from `@dcl/one-ui/text-styles.module.css` as a CSS Module. Components and the CLI remain separate from these browser exports.

The design values are generated from the verified capture pinned by `upstream.lock.json`, preserving its Dark semantic mode and four responsive layout modes. These outputs do not certify parity with later live design changes. The five families use four source pages: Colors (`1:5238`), Icons (`1:5239`), Typography (`1:5237`) and Layout & Spacing (`15521:20146`, shared by layouts and spacings). No live source is written into the library.

`semantic` exposes resolved Dark-mode values for inspection. Use `theme.color` for semantic CSS references. A text style's `fontSize` references the responsive CSS variable; `fontSizePx` is the captured tablet-and-larger value for responsive headings, not the current viewport measurement. Use the generated CSS classes for rendering and computed styles for viewport-specific pixel measurements.

Consumers configure `@dcl/one-ui` and its CSS subpaths in their TypeScript and bundler aliases before importing this source library. Application adoption, including alignment of existing Inter font registrations, is separate from this package.

Inter variable normal faces (weights 100–900) come from `@fontsource-variable/inter` 5.3.0 under the SIL Open Font License in `foundations/typography/fonts/OFL.txt`. `foundations/typography/font-source.json` pins their materialized payloads; Git LFS stores the WOFF2 assets. Font ownership is independent of the Figma capture. Do not remove the font faces or replace them with a network font dependency.

Monochrome icons inherit `currentColor`, and active-state pairs expose `active`. Icons preserve their captured intrinsic dimensions unless `size` is supplied. Give informative icons an accessible name with SVG attributes such as `role="img"` and `aria-label`; mark decorative icons `aria-hidden="true"`. Skipped source records are listed in `skippedIcons` rather than silently substituted.

## Work on it

Node 24 and pnpm (`corepack enable`). The Figma capture is not in this repository: it lives in [figma-design-system](https://github.com/decentraland/figma-design-system) (FDS). `upstream.lock.json` pins two FDS commits: `revision`, the code base every ported file is compared against, and `snapshot.revision`, the capture `fetch-source` reads, with its digest table.

```sh
cd library/ui
git lfs pull --include "library/ui/foundations/typography/fonts/*.woff2"
pnpm install --frozen-lockfile
pnpm format:check && pnpm lint && pnpm typecheck && pnpm test
node figma/lock-digests.mjs  # ported tooling and public foundations match their pinned bytes and origins
pnpm review                  # Review gallery on http://localhost:6019/dcl-ui-review/
pnpm review:build --base <BASE>  # static Review; BASE is / (default), ./, or an absolute nested prefix
pnpm review:check-build      # the built gallery navigates and renders in Chromium
```

## The capture lives in figma-design-system

The Review is a gallery of the generated components only. It publishes no Figma imagery: no reference screenshots, no fidelity comparison and no exported image fill (image fills render as a neutral placeholder). Comparing generated output with the real capture is done where the capture lives, so the guard is split in two halves:

- **In every pull request here** (public, no credential): `lock-digests.mjs` fails when any tracked file under `figma/` differs from, is missing from, or is not pinned in `upstream.lock.json`. Each pinned file also records its figma-design-system counterpart, and a file that differs from it, or has none, must name deviation codes from `deviationCodes`, each linked to the `changes` entry that gives the reason (the code `note` carries a short per-file reason instead). Nothing under `figma/` changes without a lock change.
- **When the lock changes** (a port from FDS): `port-verification.mjs` requires `port-verification.json`, the record of the real-capture checks (`fetch-source`, `deviations`, `diff`, `check-mappings`, `review-manifest`, `navbar-bounds`) for this exact lock, FDS commit and `figma/` tree, all passed. A lock change without a matching record fails; it is never skipped. Pull requests that leave the lock alone are not affected.

Maintainers with access to FDS run those checks locally:

```sh
pnpm figma fetch-source      # materialize the pinned capture into the gitignored figma/figma-snapshot/
pnpm figma diff              # generated output matches the capture (exit 1 if not)
pnpm figma check-mappings    # every *.figma.ts matches the library and the capture
```

`fetch-source` fetches exactly the pinned commit over SSH into a temporary repository, rewrites each file into the keyless form this tree reads, and accepts it only when every file matches the digest table in the lock and every ported file's recorded counterpart matches the pinned commit; the raw objects are deleted before it returns. Every command that reads the capture fails, naming `pnpm figma fetch-source`, when the cache is missing, stale or edited. A port bumps the pinned revision, runs `pnpm figma fetch-source --seal` and `node figma/lock-digests.mjs --write`, runs the checks above and commits their record. Promotion and the fidelity evidence belong to FDS.

## The Figma file key is private

The key of the Figma file never enters the repository, a generated file, a log or an artifact.

- Live Figma access is for local inspection only: `diff --source rest`, the MCP read tools with `source: "rest"`, `figma_screenshot` (the PNG is returned to the caller, never written) and `probe` read the live file with `FIGMA_TOKEN` and `FIGMA_FILE_KEY` from the environment only. Shipped output comes from the pinned capture only: `sync` and every command that writes tracked files read the verified fetch-source cache and refuse the live source. No CI job runs live code. The key is never printed: errors redact it, and the key guard (`key-guard.mjs`) scans for leaks; without the variable its `key` check reports SKIPPED while the URL checks still run.
- Snapshot ingest and `fetch-source` drop `fileKey` fields and Figma file URLs; API errors and reports redact the key.
- Mappings name the file as `<DCL_FIGMA_FILE>`; Code Connect resolves it with `documentUrlSubstitutions` at publish time only, outside this tree's code.
- `make figma-key-guard` (or `node library/ui/figma/key-guard.mjs`) fails when the key (`key` check, needs `FIGMA_FILE_KEY`) or any Figma file URL (`url` check) is in the working tree, in a blob of `origin/main..HEAD`, or in a directory passed with `--path`. It prints paths and check IDs only. Run it before every commit and push; CI runs it on every pull request and before every artifact upload. `figma/key-guard-allow.json` pins pre-existing public URLs by path and Figma file id, so any other URL added to those files still fails; the `key` check cannot be allowlisted.
- The privileged key scan (`ui-library-key-scan.yml`) reads the URL allowlist from the default branch, never from the scanned commits, so a pull request that adds a public Figma URL together with its `key-guard-allow.json` entry fails that scan until the entry lands on main.

## From Figma to the library

1. **Capture**: in figma-design-system, which owns the capture, its visual evidence and the fidelity review ([`figma/MAINTENANCE.md`](figma/MAINTENANCE.md)). This tree reads it through `pnpm figma fetch-source`.
2. **Generate** (`pnpm figma sync`): Review previews in `figma/review/src/generated/`. Never edit them by hand.
3. **Review**: `pnpm review:build` and `pnpm review:check-build` prove the gallery builds and renders; visual comparison with Figma happens in figma-design-system.
4. **Promote**: the promotion seal, its verification and `promote-reviewed` belong to figma-design-system; this tree keeps only the eligibility rule `check-mappings` reports (`figma/src/promotion/eligibility.ts`).

A synced component with the name of a hand-written one (Button, Modal) replaces it only when `promotion.include` names it explicitly (promotion, in figma-design-system, then deletes the hand-written directory and its exports; as a preview dependency or through the all-eligible default, the same name fails promotion). Once `promotion.include` names it, `check-mappings` reports the hand-written mapping as `superseded-by-sync` (eligible) or `superseded-pending` (with the reasons promotion still skips it) instead of validating it; until then the hand-written mapping is validated as usual, even when a ready page generates an eligible component with its name. Name a hand-written component in `promotion.include` only after every consumer works with the synced API.

Promoted previews reproduce the reviewed design; they do not implement production keyboard, focus or navigation semantics. A consumer composing them owns that behavior and its tests.

Generated styles use three cascade layers, declared in this order at the top of every generated stylesheet: `dcl.reset` (the runtime's base styles), `dcl.foundations` (tokens and text styles) and `dcl.components` (component styles). The cascade decides between them whatever order a bundle loads the files in, and a consumer's unlayered CSS overrides all three.

Font contract: text styles name Inter. Faces are provided by the consumer: the Review app loads `@fontsource/inter` (400/600/700); library consumers get them from the foundations stylesheet. Promotion never removes a foundations stylesheet, its font stylesheet or font files. `figma/review/test/runtime-cascade.test.ts` checks in Chromium that the NavBar label is drawn with the loaded Inter web font under both stylesheet orders.

A generated component throws during render when it receives a variant combination Figma never drew, which only an untyped caller (plain JavaScript, spread props) can pass. The error names the component and every axis value, for example `NavBar has no captured view for viewport="largeDesktop", balances="credits", theme="dark", signedIn=false`. This is decided by the owner: throw in all environments, server rendering included; there is no fallback view. A consumer that renders generated components from data it does not type-check should place them under a React error boundary, and let server rendering surface the error through the route's error handling (for example a framework `ErrorBoundary` route export) instead of catching and hiding it.

`figma/figma.config.json` can bind named text roles to captured text layers (`components.<Figma name>.textRoles`): each role is a public prop bound by Figma node id, and each node states its expected path of layer names, so generation fails on an unknown id or a renamed path instead of binding the wrong text. A `string` role takes text. A role whose text sits inside a linked action declares that link's prop name with `href` (for example `"discoverLabel": { "type": "string", "href": "discoverHref", ... }`): the generator emits exactly that name, and generation fails if it is missing or two roles declare the same one. A `count` role takes a non-negative integer and needs its `label` role together with it; the component throws on any other count value or a missing label. The label becomes the accessible name of the action around the count and replaces its captured name, so it must state the count itself, localized by the consumer (for example `notificationCount={12}` with `notificationsLabel="12 notifications"`). Binding needs node ids in the capture, which the current capture does not record yet, so no component binds roles today.

## Provenance

`upstream.lock.json` records the imported sources, their revisions and licenses, the digest of every ported file and of every file of the pinned capture.

This tree ships the pinned `9c6dfbe6` capture. Reported live differences in colors (89 primitives and 42 semantics), text styles (34 rather than 32), spacing (18 rather than 16) and body/sm line height await a coordinated capture refresh. They are not included until the capture is refreshed and foundations are regenerated.

### Regenerate foundations

Maintainers run `pnpm figma fetch-source` to materialize the pinned cache, then `pnpm figma install foundations`. This single install boundary regenerates the five families and `source.json`, preserves maintained CSS and licensed assets, and refuses unverified input or dirty targets. For an isolated reproduction, use `pnpm figma install foundations --out ../.agent-work/figma-install/proof/foundations`. Repeating installation makes no changes.
