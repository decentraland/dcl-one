# Social voice schema release definitions

This directory contains the sole canonical definitions used by
[`v3-private-voice-pending.sql`](../v3-private-voice-pending.sql). A version-2
schema build does not discover nested release definitions during fresh
bootstrap or migration. The version-3 build explicitly enrolls these same
definitions once in dependency order, alongside its compatible runtime.

The release accepts captured shipped-main version 2 without historical step
0020 and prepared version 2 with its genuine matching receipt. It refuses
mixed layouts, changed receipts, an existing destination and overlapping or
non-normalized pending wallets. It retains call, participant and credential
facts, real ledgers and the remaining notification TTL and generation.
Historical UUID columns cannot recover raw identifier aliases already lost.

`services/catalyst/db/tests/social_voice_release.rs` exercises both genuine
origins, populated rows, rollback boundaries, refusal and retry. The separate
`social_voice_upgrade.rs` suite verifies the current version-3 bootstrap, real
migrator and immutable historical-step retirement against both genuine origins.
A version-2 binary refuses a released version-3 database. Release-only tests
do not establish compatible runtime behavior or deployment readiness.

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
