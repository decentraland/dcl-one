# ADR: Dormant schema release definitions

- Status: Proposed
- Date: 2026-11-04

## Context

The version-2 schema build discovers top-level SQL and additive migrations.
Enrolling future Social voice definitions there changes fresh databases before
the release and compatible runtime are available. Copying CREATE bodies into
a release script would give the same facts two independently editable owners.
The [database contract](../../DATABASE.md) requires a separate reviewed release
for destructive changes and one fresh-schema bootstrap.

## Proposal

Keep the canonical future definitions under `schemas/releases/v3/`. The
release includes them directly. Version-2 builds leave that nested directory
dormant; a compatible version-3 build explicitly enrolls the same files in its
single fresh rendering, in dependency order, exactly once. No second schema
bootstrap, runtime schema-target flag or duplicated CREATE body is introduced.

Release artifacts and their genuine populated upgrade tests can be reviewed
separately from production activation and compatible writers. The old binary
continues to require version 2 and refuses version 3. Artifact review or merge
does not authorize applying the release or establish cutover readiness.

## Consequences

The version-3 activation must keep explicit enrollment, release includes,
immutable historical migration checks and runtime writers consistent. Both
genuine version-2 origins, real ledgers, populated facts and rollback boundaries
require executable evidence. Backup restoration and deployed cutover remain
operating gates. This proposal records no acceptance or rollout approval.
