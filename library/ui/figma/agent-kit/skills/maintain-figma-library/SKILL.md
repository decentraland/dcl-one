---
name: maintain-figma-library
description: Use when porting a figma-design-system capture into library/ui, refreshing the generated foundations or components, acting on an "Unresolved component" report from the implement-figma-design skill, adding a public component mapping, or answering why a component is needs-manual. Triggers on "port the capture", "refresh library/ui from Figma", "port prepare", "port verify", "port-verification.json", "needs-manual", "add this component to the library".
---

# Maintain the Figma-generated library

You change `library/ui` from the capture that figma-design-system (FDS) owns. You need read access to FDS. Everything runs from `library/ui` with Node 24.

## 1. Intake

- **A consumer report** ("Unresolved component": component, code, variant properties, library version, capture revision). A `mapping-missing` component is a known library component without a public mapping. An identity code (`identity-not-found`, `identity-ambiguous`, `identity-uncertain`) is not a missing mapping: check the name and the capture first.
- **A capture refresh**: the new FDS commit to pin.

Before any work, report the `needs-manual` flags and their reasons for the components involved (`pnpm --silent figma diff` and the sync report list them; `figma/MAINTENANCE.md` explains each reason). Never clear a flag to make a count look better. Extractor limits behind a flag (a truncated capture, an unresolved instance) belong to FDS: report them there instead of patching around them here. Generation produces the Review previews and the NavBar bridge only. A new public component, or one a consumer report asks for, is written by hand from the template in `library/ui/templates/component/` following `docs/ui-standards.md`, with its own `*.figma.ts` mapping; never add it to `components/promotion.json` or `promotion.include` in `figma/figma.config.json`.

## 2. Prepare

For a capture refresh, first point the lock at the new capture: set `revision` and `snapshot.revision` of the figma-design-system source in `upstream.lock.json` to the new FDS commit, then run `pnpm figma fetch-source --seal`, which fetches that commit and writes its digest table. Skip this when you only regenerate from the capture already pinned. Without it, `port prepare` verifies against the old digest table and either fails at fetch-source or prepares the old capture again.

```sh
pnpm figma port prepare
```

It refuses a `library/ui` with changes other than `upstream.lock.json` (commit or remove them first), then runs fetch-source, diff, sync, a second sync that must change nothing, install foundations, install components and `lock-digests --write` (it stages `library/ui` first, because lock-digests reads the index), in that order, and stops at the first stage that fails, naming it. Then record a deviation code for every changed file under `figma/` in `upstream.lock.json` (`sources[0].deviationCodes`, a stable id mapped to its reason, listed in that file's `deviation`), stage `library/ui` again (`git add -A -- library/ui`), rerun `node figma/lock-digests.mjs --write` (it reads the index), and commit.

## 3. Verify

```sh
pnpm figma port verify
```

It refuses a tree with uncommitted changes, runs the seven checks (fetch-source, deviations, diff, check-mappings, review-manifest, navbar-bounds, install) against the verified capture, and only when all pass writes `library/ui/port-verification.json` with each check's own counts. A failed check writes no record, leaves an existing one as it was and puts back only what the checks regenerate (the Review under `figma/review/`, `foundations/`, `components/`), after copying those files to `.agent-work/figma-port/restored-<time>/`, so the tree stays clean for the next run. Any other change under `library/ui`, such as an edit you made while it ran, is left as it is and listed as kept. The record is written only when HEAD did not move during the checks and the head passes the exemption scan CI runs. Commit the record. `node figma/port-verification.mjs --print-hash` prints the hash the record must name.

The record states what your local run reported. CI checks that it names the head's port inputs and that every result is `pass`; it is not proof that the checks ran, an independent review, an attestation or evidence of live Figma parity. Say so in the pull request, and ask for review of the exact commit proposed for merge.

## 4. A new public Figma URL

The privileged key scan reads the URL allowlist from the default branch. A public Figma URL therefore lands in two pull requests: first its `figma/key-guard-allow.json` entry, merged to main; then the change that adds the URL. Never put a file key, a component key or a private URL anywhere in the tree, a log or a pull request.

## 5. Hand back

Push, read CI, and get CODEOWNERS review at the exact SHA. Then tell the contributor which components now resolve (`pnpm --silent figma resolve --mapping-only --name '<set name>' --variant 'Prop=Value'`; a set name and its values come from the design, so keep them in single quotes and write a `'` inside as `'\''`) so they can rerun the consumer skill.
