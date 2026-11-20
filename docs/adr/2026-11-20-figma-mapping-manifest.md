# ADR: Hand-written mappings are checked against a tracked manifest

- Status: Proposed
- Date: 2026-11-20

## Context

Every public component in `library/ui/components` is written by hand and carries a `*.figma.ts` mapping that names its Figma node. `check-mappings` verified those mappings against the page capture pinned in `library/ui/upstream.lock.json`, which lives in another repository. The check therefore ran only on a machine that could fetch that capture, never in pull request CI, and a mapping whose page was not in the capture failed with no way to fix it from this repository.

The check reads little of a capture: for each mapped node, its name, its kind, its property definitions and its variants.

## Decision

`library/ui/figma-mapping-manifest.json` holds exactly that data for the nodes the mappings name. `pnpm figma manifest refresh` writes it from the live file with `FIGMA_TOKEN` and `FIGMA_FILE_KEY`; `pnpm figma manifest verify` compares it with the live file and writes nothing. `check-mappings` reads the manifest, fails on a mapping it does not hold and on an entry no mapping names, and runs in `ui-library.yml` on every pull request without a capture or credentials.

The manifest holds node ids and plain names. Its parser refuses a Figma URL, a key-shaped string, a repeated node and any field outside its schema, and the key guard scans it like every tracked file.

The manifest is outside `library/ui/figma`, so it is not a port input: refreshing it needs no port verification record. The record keeps its seven checks; its `check-mappings` entry now reports a run against the manifest.

## Consequences

- A mapping and its manifest entry land in the same pull request, and CI enforces that they agree.
- Mapping a component no longer needs its page in the pinned capture.
- The Review gallery and the foundations still come from the pinned capture. A mapped component can be absent from the gallery.
- The manifest states what Figma defined when it was refreshed. `manifest verify` is the drift check and needs credentials, so it runs by hand or from a trusted workflow, never on a pull request.
