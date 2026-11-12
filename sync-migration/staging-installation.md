# Staging source installation

The package supports **source-only production → dcl-one capture** for scoped Places catalogue/interactions, Events schedules and social settings. No client, gateway or source HTTP-handler PR is required. The source databases do need an explicit capture installation: application, internal and scheduled SQL writes otherwise have no durable, ordered record for the worker to consume.

This is a staging procedure, not evidence that production roles, deployed schemas or capacity have been verified. Use a dedicated target with normal serving, jobs, mirrors and providers disabled. Shop and credits remain excluded. Source remains the sole owner of effects.

## Inputs and boundaries

Record the exact source/target commits, PostgreSQL version, selected UUIDs/wallets, every application/job/operator writer role, a dedicated reader role, installation owner and recovery operator. Compare the deployed schema with the reviewed mapping before installation. New schedules/place IDs and wallets outside the immutable allowlists are not captured.

The [Places](../services/migration-sync/source/places.sql), [Events](../services/migration-sync/source/events_schedules.sql) and [social-settings](../services/migration-sync/source/social-settings.sql) adapters use invoker functions. Every writer therefore needs capture permissions. Granting only the HTTP service role is insufficient when a job or script uses another role. Scope/control/history are trusted infrastructure; source writers can update the required capture metadata and are not isolated from one another by these grants.

The capture reader is a dedicated unprivileged role with no role memberships. These scripts grant table-wide SELECT, not row-level isolation; scoped mapping does not prevent that role from reading other rows in those tables. Keep credentials restricted accordingly. Function permissions assume exactly the reviewed adapter SQL. Preflight checks flags and expected trigger counts, but does not attest function bodies, column types or detect arbitrary malicious replacement code.

## Installation sequence

1. Fence **all** source writers for the relevant tables, including jobs, queues and manual scripts, and drain active write transactions. Prepare credentials through the deployment secret mechanism; never paste connection strings into tracked files or logs.
2. Run the explicit worker `install-source` or `install-domain-source` command with the installation owner and reviewed allowlist, as shown in the [worker runbook](../services/migration-sync/README.md). The installer commits triggers and scope together. Do not run the raw adapter SQL alone: that omits scope installation and installer validation. Existing older adapters require a reviewed new baseline; never relabel a cursor.
3. While writers remain fenced, apply [grants.sql](../services/migration-sync/source/grants.sql) for **each** writer role. Repeat the [preflight](../services/migration-sync/source/preflight.sql) for each writer/reader pair. The installer does not grant runtime roles; releasing writers between installation and grants can break their transactions.
4. Exercise representative scoped and unscoped writes under every real writer role in staging. Confirm committed changes appear exactly once; rollback changes disappear; missing grants fail the business transaction rather than silently losing history. Exercise mixed transactions, deletes and the actual job SQL. Test TRUNCATE rejection and source lock contention.
5. Resume source writers only after grants and checks pass. Bootstrap with the dedicated reader and isolated target credentials. Bootstrap briefly fences source writes while establishing its repeatable-read snapshot; capture bridges commits made after that boundary. Capture and target application then run as separate processes/credentials.
6. Reconcile each source, pause application while capture continues, restart workers and rehearse source/target outages. Measure lag, lock wait, retained history growth and catch-up speed. Set the duration, budgets and alert ownership before a sustained soak. See [rollout](rollout.md).

Use a libpq service definition or environment supplied by the deployment secret mechanism. `PGSERVICE` below selects an operator-configured **staging** database; role names are explicit inputs, not defaults:

```sh
: "${PGSERVICE:?Select the reviewed staging libpq service first}"
psql -X -v ON_ERROR_STOP=1 \
  -v source=places -v writer_role="$SOURCE_WRITER_ROLE" -v reader_role="$CAPTURE_READER_ROLE" \
  -f services/migration-sync/source/grants.sql
psql -X -v ON_ERROR_STOP=1 \
  -v source=places -v writer_role="$SOURCE_WRITER_ROLE" -v reader_role="$CAPTURE_READER_ROLE" \
  -f services/migration-sync/source/preflight.sql
```

For the other source databases use `source=events` or `source=social-settings`. Run these as the installation owner, against one source database at a time. The scripts quote identifiers, reject unknown adapters/missing roles, do not create login roles and stop on errors. Grants are transactional and repeatable. The preflight makes no persistent writes and exits unsuccessfully on missing rights, invalid control/scope, disabled expected triggers, reader mutation rights, RLS, privileged reader flags or reader membership/ownership that defeats the intended isolation. Inspect inherited/custom grants beyond these captured objects separately; this is not a whole-database security audit.

The source-only grants intentionally revoke PUBLIC access to capture objects and function execution, and do not grant the optional Places `execute_favorite`/`record_command` helpers or receipt writes. They do not revoke pre-existing explicit role grants. Do not apply them blindly to an already active native-replay deployment. That mode needs its own reviewed grants and command-receipt tests.

## Pause, abort and retire without deleting evidence

Pausing target application or stopping the worker does **not** disable source capture. Leave triggers, scopes, control, history and permissions intact so writes continue to be retained. A stopped reader performs no source acknowledgement and does not prune history. Monitor source storage during a pause.

If installation fails, keep writers fenced until the entire adapter and grants are either repaired or deliberately retired. Do not disable a trigger just to make a failed write succeed while treating its stream as continuous.

For permanent retirement, fence source writers, record the final generation/position and target checkpoint, drain/reconcile or explicitly abandon the target, then stop capture/application. Archive source/target history for the agreed recovery horizon. In a separately reviewed database change, remove **only the installed capture triggers from the listed source tables**, retaining the capture schema/history and an external retirement record. Check dependent functions before revoking per-writer capture rights. Resume source writes only after verifying ordinary writes work without capture. Once any uncaptured write occurs, that generation cannot resume: a new reviewed capture installation and baseline are required. No automatic DROP SCHEMA, history deletion or trigger-disabling command is included in this package.

## Local validation

[The disposable role test](../services/migration-sync/source/test_roles.py) installs all three adapters with separate non-owner writer/reader roles, runs writer inserts/updates/deletes, verifies reader SELECT and denied mutations/functions, and checks that missing grants, disabled triggers, reader role-assumption membership, schema ownership and column-only UPDATE grants fail preflight. It uses `char(36)` Places IDs and runs only against a localhost PostgreSQL URL supplied through `MIGRATION_SYNC_TEST_DATABASE_URL`. It creates and removes uniquely named fixture databases/roles. It does not validate actual deployed roles, production jobs, failover or throughput.
