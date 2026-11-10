# ADR: The port verification record binds port inputs, not the whole tooling tree

- Status: Accepted
- Date: 2026-11-10

## Context

The checks that compare generated UI output with the real Figma capture run only where the capture is readable, so a port into `library/ui` carries `library/ui/port-verification.json`, a record of those local checks, and CI verifies that the record names what the pull request contains. The record used to name the hash of the whole `library/ui/upstream.lock.json` and the Git tree of the whole `library/ui/figma` directory. Any change under that directory, including tests, test fixtures, the consumer skill text and maintenance notes, required a new record from someone with capture access, and every two branches that touched the directory conflicted on the same record lines. Deviation codes pointed at positions in the lock's `changes` list, so two branches adding a reason conflicted on the list and on the index.

## Decision

The record names a hash of the port inputs:

- the figma-design-system code and capture commits pinned in the lock, and the capture digest table;
- the lock's public foundation and component tables;
- the bytes of every file tracked under `library/ui/figma`, except tests (`*.test.*`, `*.spec.*`), anything under a `test`, `__tests__` or `fixtures` directory, and Markdown files.

A pull request needs a new record only when that hash differs from the base's. A test in `port-verification.test.ts` fails when a port input imports an exempt file or names a Markdown file, which keeps the exemption sound. The lock's provenance text (license, imported paths, narrative changes, deviation codes and counterparts) is not a port input; `lock-digests.mjs` still pins every tracked file under `library/ui/figma`, exempt files included, and validates that text in every pull request.

Each deviation code in the lock maps to its own reason, keyed by a stable id in sorted order, instead of an index into `changes`.

The record states what a local run reported. CI verifies that it names the head's port inputs and that every required check reports `pass`; it does not prove that the checks ran, and it is neither an attestation, an independent review nor evidence of live Figma parity. Trust in a port remains CODEOWNERS review and an independent review of the exact commit proposed for merge.

## Consequences

- Tests, fixtures, the skills and maintenance notes under `library/ui/figma` change without capture access. They still need resealed lock digests.
- Two branches that each change a port input still conflict on the record, which is intended: the combined tree was not checked, so one of them reruns the checks after rebasing.
- Record schema 1 is no longer accepted. The pull request that introduces schema 2 carries a schema 2 record produced by a local run.
- Run evidence stays in Git. Moving it to an exact-commit CI lane would need a new credential with capture access and is not part of this decision.
