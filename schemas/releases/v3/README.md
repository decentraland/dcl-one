# Social voice schema release definitions

This directory contains the sole canonical definitions used by
[`v3-private-voice-pending.sql`](../v3-private-voice-pending.sql). A version-2
schema build does not discover nested release definitions during fresh
bootstrap or migration. The version-3 build explicitly enrolls these same
definitions once in dependency order, alongside its compatible runtime.

The release accepts captured shipped-main version 2 without historical step
0020 and prepared version 2 with its genuine matching receipt. It refuses
mixed layouts, changed receipts, an existing destination and overlapping or
non-normalized pending wallets. Both origins must have the expected named
version-2 uniqueness constraints and no extra unique indexes on those relations.
The legacy notification scope, generation and expiry must form a complete
tuple before any retained facts change. It retains call, participant and credential
facts, real ledgers and the remaining notification TTL and generation.
Historical UUID columns cannot recover raw identifier aliases already lost.

`services/catalyst/db/tests/social_voice_release.rs` exercises both genuine
origins, populated rows, rollback boundaries, refusal and retry. The separate
`social_voice_upgrade.rs` suite verifies the current version-3 bootstrap, real
migrator and immutable historical-step retirement against both genuine origins.
A version-2 binary refuses a released version-3 database. Release-only tests
do not establish compatible runtime behavior or deployment readiness.

The release sets `ON_ERROR_STOP` before opening its transaction. Execute it
with `psql -X -f schemas/releases/v3-private-voice-pending.sql` so relative
includes resolve from the release file. A missing include stops the client;
disconnecting rolls back the open transaction. Before stamping version 3, the
release verifies the destination and both replacement indexes, including their
key definitions and validity. Tests run the real `psql` client with missing
includes, default and disabled ambient `ON_ERROR_STOP`, and standard-input
execution; every failed attempt must retain the complete version-2 snapshot.

```sh
CATALYST_TEST_DATABASE_URL=postgres://test_role@localhost/test_admin \
  cargo test --manifest-path services/catalyst/Cargo.toml \
  -p catalyst-db --test social_voice_release --locked
```

The test role must be allowed to create and drop its own disposable databases.
Source manifests beside the fixtures pin the two historical inputs; capture
tools preserve genuine schema receipts rather than synthesizing upgrade history.

The `Social voice release proof` workflow runs this suite with an explicit
database URL. Its `tooling/social-voice-release-test.sh` runner creates an owned
PostgreSQL 18.6 cluster using Unix sockets, verifies connection and database
creation privileges, and stops and removes that cluster on exit. Run it through
`nix develop --no-update-lock-file --command bash tooling/social-voice-release-test.sh`.
The runner ignores ambient database settings; infrastructure tests exercise
setup failures, test failures and cleanup. A run without a database is not
release evidence.

Merging these dormant artifacts does not execute SQL or establish deployment
readiness. Before a cutover, provide a compatible artifact, stop every writer,
poller and administrative write path, verify backup restoration, execute the
reviewed release, and verify the compatible migrator, schema status and runtime
before traffic resumes. An old version-2 binary refuses version 3. Restoring
old constraints by deleting or deduplicating retained facts is not a rollback.

The layout proposal is recorded in the
[release-definition ADR](../../../docs/adr/2026-11-04-dormant-schema-release-definitions.md).
