# Social voice schema release definitions

This directory contains the sole canonical definitions used by
[`v3-private-voice-pending.sql`](../v3-private-voice-pending.sql). The current
version-2 binary does not discover nested release definitions during fresh
bootstrap or migration. Adding these artifacts leaves its required version,
fresh stamp and enrolled additive steps unchanged. A compatible version-3
artifact must explicitly enroll these same definitions once.

The release accepts captured shipped-main version 2 without historical step
0020 and prepared version 2 with its genuine matching receipt. It refuses
mixed layouts, changed receipts, an existing destination and overlapping or
non-normalized pending wallets. It retains call, participant and credential
facts, real ledgers and the remaining notification TTL and generation.
Historical UUID columns cannot recover raw identifier aliases already lost.

`services/catalyst/db/tests/social_voice_release.rs` exercises both genuine
origins, populated rows, rollback boundaries, refusal and retry. Its version-2
executor checks establish that fresh bootstrap is unchanged and that the old
binary refuses a released version-3 database. Production version-3 bootstrap,
migration retirement and compatible writers require their own activation
checks; these release-only tests do not establish them.

```sh
CATALYST_TEST_DATABASE_URL=postgres://test_role@localhost/test_admin \
  cargo test --manifest-path services/catalyst/Cargo.toml \
  -p catalyst-db --test social_voice_release --locked
```

The test role must be allowed to create and drop its own disposable databases.
Source manifests beside the fixtures pin the two historical inputs; capture
tools preserve genuine schema receipts rather than synthesizing upgrade history.

Merging these dormant artifacts does not execute SQL or establish deployment
readiness. Before a cutover, provide a compatible artifact, stop every writer,
poller and administrative write path, verify backup restoration, execute the
reviewed release, and verify the compatible migrator, schema status and runtime
before traffic resumes. An old version-2 binary refuses version 3. Restoring
old constraints by deleting or deduplicating retained facts is not a rollback.

The layout proposal is recorded in the
[release-definition ADR](../../../docs/adr/2026-11-04-dormant-schema-release-definitions.md).
