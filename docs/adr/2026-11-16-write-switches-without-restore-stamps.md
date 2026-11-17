# ADR: Write switches without restore stamps

- Status: Accepted
- Date: 2026-11-16

## Decision

Each of the six write switches (`CONTENT_WRITES_ENABLED`, `CREATE_WRITES_ENABLED`, `DATA_WRITES_ENABLED`, `SOCIAL_WRITES_ENABLED`, `WORLDS_WRITES_ENABLED`, `OPS_WRITES_ENABLED`) alone decides whether its plane accepts writes. A switch declares that this node is the authority for that domain; its routes, jobs and authority semantics follow it.

The node no longer reads restore stamps (`backup.json`, `drill.json`) or refuses writes or startup over their age or blob coverage. `WRITES_RESTORE_STAMP_DIR`, `WRITES_BACKUP_MAX_AGE_HOURS` and `WRITES_DRILL_MAX_AGE_DAYS` are removed, and `/writes` reports only `planes`.

Durability is the operator's responsibility: a node taking local writes holds the only copy of that data, so its deployment backs up the database, the blob store (`STORAGE_ROOT_FOLDER`) and the credentials, and tests restores, with the platform's own backup and restore tooling. The app does not enforce it.

## Reasons

The stamps made the app judge a backup it cannot observe: it trusted two files written by deployment scripts, including a self-declared blob coverage flag, and held writes on their age. A missed or slow backup job closed every plane for a day, and the check tied the app to one deployment's backup layout. Managed backup services report and alert on their own jobs.

## Validation

The shared write-gate tests cover each plane open exactly when its switch is on and the `/writes` document. Posture tests cover mirror and island starting with switches on and no stamp settings. Domain tests cover each write route answering 503 with its disabled message while its switch is off.
