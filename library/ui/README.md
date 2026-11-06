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

## Provenance

`upstream.lock.json` records the imported sources, their revisions and licenses.
