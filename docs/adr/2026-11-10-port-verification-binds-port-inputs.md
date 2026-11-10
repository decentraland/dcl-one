# ADR: The port verification record binds port inputs, not the whole tooling tree

- Status: Accepted
- Date: 2026-11-10

## Context

The checks that compare generated UI output with the real Figma capture run only where the capture is readable, so a port into `library/ui` carries `library/ui/port-verification.json`, a record of those local checks, and CI verifies that the record names what the pull request contains. The record used to name the hash of the whole `library/ui/upstream.lock.json` and the Git tree of the whole `library/ui/figma` directory. Any change under that directory, including tests, test fixtures, the consumer skill text and maintenance notes, required a new record from someone with capture access, and every two branches that touched the directory conflicted on the same record lines. Deviation codes pointed at positions in the lock's `changes` list, so two branches adding a reason conflicted on the list and on the index.

## Decision

The record names a hash of the port inputs:

- the whole lock except its provenance text: each source's `license`, `imported`, `changes` and `deviationCodes` (the reasons), and per-file `note`s. Any other field, including one added later, is bound;
- the bytes of every file tracked under `library/ui/figma`, except `*.test.*` and `*.spec.*` files, anything under `library/ui/figma/test/` or `library/ui/figma/review/test/`, and Markdown files. A component generation input (`component-generation.mjs`) is never exempt, and neither is `port-verification.test.ts`, which holds the scan that keeps the exemption sound. Both scripts read paths from `git -z` listings up to the first tab only as metadata, refuse a path with a control character, a path listed twice, and any mode other than a regular or executable file, so a sibling path or a symlink cannot stand in for a port input.

An exempt file's lock entry stays bound without its own digest and deviation codes. Its FDS counterpart is bound because `fetch-source`, a required check, compares every recorded counterpart with the pinned FDS commit, and `lock-digests.mjs` can only check that the lock agrees with itself. Adding, removing or re-pointing any file therefore needs a new record; editing an existing test, fixture or Markdown file does not.

A pull request needs a new record only when that hash differs from the base's. `lock-digests.mjs` still pins every tracked file under `library/ui/figma`, exempt files included, and checks that each differing file names a declared deviation code. `port-verification.mjs` itself, in the required `key-guard` job, first scans every head: it fails when the code of a port input has a string literal that names an exempt file (a relative path into a test root, a test or spec basename, a Markdown name, a test directory segment, ignoring query strings, fragments and case), and when the validator imports anything beyond `./component-generation.mjs`, `./lock-digests.mjs` and Node built-ins. The scan is a tripwire, not a proof: paths built by concatenation or at run time are not seen (the agent kit, for example, copies its skill directories with `readdir`, and no check against the capture runs it), so review still owns that boundary. The generation inputs outside `library/ui/figma` (`library/ui/package.json`, `pnpm-lock.yaml`, `tsconfig.json`) are not hashed directly; they are bound through `components/generation.json`, whose bytes the lock's public component table pins.

Each deviation code in the lock maps to its own reason, keyed by a stable id in sorted order, instead of an index into `changes`. Sorting makes two branches that add different codes rarely touch the same lines; it does not remove conflicts on per-file entries.

The record states what a local run reported. CI verifies that it names the head's port inputs and that every required check reports `pass`; it does not prove that the checks ran, and it is neither an attestation, an independent review nor evidence of live Figma parity. Trust in a port remains CODEOWNERS review and an independent review of the exact commit proposed for merge.

## Consequences

- Edits to existing tests, fixtures, the skills and maintenance notes under `library/ui/figma` need resealed lock digests and no new record. Adding or removing such a file needs a new record.
- Two branches that each change a port input still conflict on the record, which is intended: the combined tree was not checked, so one of them reruns the checks after rebasing.
- Record schema 1 is no longer accepted. The pull request that introduces schema 2 carries a schema 2 record produced by a local run.
- Run evidence stays in Git. Moving it to an exact-commit CI lane would need a new credential with capture access and is not part of this decision.
