# @dcl/one-ui

Shared UI foundations and components for dcl-one, and the Node-only Figma tooling that produces them.
The standards they follow are in [`docs/ui-standards.md`](../../docs/ui-standards.md).

| Path                   | What it is                                                                                                                                                                                                                                                         |
| ---------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `figma/`               | Node-only: CLI and MCP server (`dcl-figma`), saved capture (`figma-snapshot/`), Review gallery (`review/`), consumer skill kit (`agent-kit/`), `key-guard.mjs`. Never imported by browser code. Maintenance notes: [`figma/MAINTENANCE.md`](figma/MAINTENANCE.md). |
| `templates/component/` | Starting point for a new shared component.                                                                                                                                                                                                                         |

## Work on it

Node 24 and pnpm (`corepack enable`). Clone with Git LFS: the Figma capture, Review references and evidence, generated Review SVG assets, the promotion manifest and the font files are LFS objects, and the tooling fails when it meets a pointer instead of content.

```sh
cd library/ui
pnpm install --frozen-lockfile
pnpm format:check && pnpm lint && pnpm typecheck && pnpm test
pnpm figma diff              # generated Review output matches the capture (exit 1 if not)
pnpm figma check-mappings    # every *.figma.ts matches the library and the capture
pnpm figma promotion-verify  # the sealed promotion manifest still matches its reviewed inputs
pnpm review                  # Review gallery on http://localhost:6019/dcl-ui-review/
pnpm review:build --base <BASE>  # static Review; BASE is / (default), ./, or an absolute nested prefix
```

## The Figma file key is private

The key of the Figma file never enters the repository, a generated file, a log or an artifact.

- Live Figma access reads it from `FIGMA_FILE_KEY` (with `FIGMA_TOKEN`). `sync`, `diff`, `check-mappings`, promotion and Review work from the saved capture without it.
- Snapshot ingest drops `fileKey` fields and Figma file URLs; API errors and reports redact the key.
- Mappings name the file as `<DCL_FIGMA_FILE>`; Code Connect resolves it with `documentUrlSubstitutions` from `FIGMA_FILE_KEY` at publish time only.
- `make figma-key-guard` (or `node library/ui/figma/key-guard.mjs`) fails when the key (`key` check, needs `FIGMA_FILE_KEY`) or any Figma file URL (`url` check) is in the working tree, in a blob of `origin/main..HEAD`, or in a directory passed with `--path`. It prints paths and check IDs only. Run it before every commit and push; CI runs it on every pull request and before every artifact upload. `figma/key-guard-allow.json` pins pre-existing public URLs by path and Figma file id, so any other URL added to those files still fails; the `key` check cannot be allowlisted.

## From Figma to the library

1. **Capture** (`figma/figma-snapshot/`): read-only from Figma, by hand, or by the full sync in figma-design-system ([`figma/MAINTENANCE.md`](figma/MAINTENANCE.md)).
2. **Generate** (`pnpm figma sync`): Review previews in `figma/review/src/generated/`. Never edit them by hand.
3. **Review**: build, capture and check the visual evidence (`pnpm review:build`, `pnpm review:capture`, `pnpm review:build`, `pnpm review:check`), then compare every changed variant with Figma. Pixel differences need human review.
4. **Seal** (`pnpm figma promotion-manifest`): hashes every reviewed input, and each eligible component spec, into `figma/review/src/review/promotion.json` (schema 2). Specs are sealed by hash rather than embedded, because their inline artwork makes the file too large; promotion rebuilds them from the reviewed capture and requires every hash to match. A manifest in the older schema 1 is reported as legacy and stale until it is resealed after review.
5. **Promote** (`pnpm figma promote-reviewed`): verifies the seal, then writes the components listed in `figma/figma.config.json` `promotion.include` (and the previews they depend on) into `components/<Name>/`, the runtime into `components/runtime/` and the foundations from the capture into `foundations/`.

A synced component with the name of a hand-written one (Button, Modal) replaces it only when `promotion.include` names it explicitly: promotion then deletes the hand-written directory, tests and mapping included, and its exports from `index.ts`. Promoted as a preview dependency or through the all-eligible default, the same name fails promotion instead. Once `promotion.include` names it, `check-mappings` reports the hand-written mapping as `superseded-by-sync` (eligible) or `superseded-pending` (with the reasons promotion still skips it) instead of validating it; until then the hand-written mapping is validated as usual, even when a ready page generates an eligible component with its name. Name a hand-written component in `promotion.include` only after every consumer works with the synced API.

Promoted previews reproduce the reviewed design; they do not implement production keyboard, focus or navigation semantics. A consumer composing them owns that behavior and its tests.

Generated styles use three cascade layers, declared in this order at the top of every generated stylesheet: `dcl.reset` (the runtime's base styles), `dcl.foundations` (tokens and text styles) and `dcl.components` (component styles). The cascade decides between them whatever order a bundle loads the files in, and a consumer's unlayered CSS overrides all three.

Font contract: the text styles name the `Inter` family, and where the faces come from depends on the context. In the Review app (and in figma-design-system) the entry loads `@fontsource/inter` 400, 600 and 700 once. In the installed `@dcl/one-ui` library, `foundations/foundations.css` imports `typography/inter.css`, which declares the bundled Inter faces in `typography/fonts`: a consumer imports `@dcl/one-ui/foundations.css` once and gets Inter with it. Promotion regenerates the token and text-style files and never removes `typography/inter.css`, the font files or the foundations entry. `figma/review/test/runtime-cascade.test.ts` checks in Chromium that the NavBar label is drawn with the loaded Inter web font under both stylesheet orders.

A generated component throws during render when it receives a variant combination Figma never drew, which only an untyped caller (plain JavaScript, spread props) can pass. The error names the component and every axis value, for example `NavBar has no captured view for viewport="largeDesktop", balances="credits", theme="dark", signedIn=false`. This is decided by the owner: throw in all environments, server rendering included; there is no fallback view. A consumer that renders generated components from data it does not type-check should place them under a React error boundary, and let server rendering surface the error through the route's error handling (for example a framework `ErrorBoundary` route export) instead of catching and hiding it.

`figma/figma.config.json` can bind named text roles to captured text layers (`components.<Figma name>.textRoles`): each role is a public prop bound by Figma node id, and each node states its expected path of layer names, so generation fails on an unknown id or a renamed path instead of binding the wrong text. A `string` role takes text. A role whose text sits inside a linked action declares that link's prop name with `href` (for example `"discoverLabel": { "type": "string", "href": "discoverHref", ... }`): the generator emits exactly that name, and generation fails if it is missing or two roles declare the same one. A `count` role takes a non-negative integer and needs its `label` role together with it; the component throws on any other count value or a missing label. The label becomes the accessible name of the action around the count and replaces its captured name, so it must state the count itself, localized by the consumer (for example `notificationCount={12}` with `notificationsLabel="12 notifications"`). Binding needs node ids in the capture, which the current capture does not record yet, so no component binds roles today.

## Provenance

`upstream.lock.json` records the imported sources, their revisions and licenses.
