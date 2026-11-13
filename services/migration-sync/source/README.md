# Places outbox capture adapter

This opt-in pilot captures native favorite/vote commands and trusted POI-job membership for an immutable list of Genesis place UUIDs. Install only with the explicit operator command after reviewing the source DDL and role grants. It is not automatically installed when the worker starts. The target catalogue must already contain the same local place IDs. Shop, treasury and credits are excluded.

`places_hybrid_commands_v1` is a transactional outbox, not logical WAL decoding. Its monotonically committed transaction ordinals use the envelope's hexadecimal cursor representation; they must never be compared to PostgreSQL WAL LSNs. Source generation and adapter identity fence distinct capture histories. A restored/replaced source must receive a new reviewed generation and baseline before capture resumes; restoring the same generation and reusing positions cannot be treated as a safe failover.

A source-local BEFORE STATEMENT trigger serializes interaction and POI mutations with a transaction advisory lock. Relevant rows and accepted command receipts share a transactional counter and commit atomically. Rollbacks consume no committed position. Internal SQL writes and deletes are captured even when they bypass HTTP, so a
missing duplicate command is visible. Capturing those changes does not authorize
applying their rows: after baseline migration, unreceipted mutations block target
progress. Only a durably accepted equivalent interaction command or the approved producer-tagged POI result can apply. TRUNCATE is rejected because it cannot preserve this snapshot/capture contract. Disabling triggers, replacing the table, changing the capture schema, or bypassing triggers invalidates the pilot and requires stopping capture and a reviewed baseline.

The POI job additionally needs its existing normalized-category permissions and access to `migration_capture.apply_poi_membership`; the explicit `MIGRATION_POI_CAPTURE_ENABLED` flag has no fallback on helper failure. See the [exact residual scope](../../../sync-migration/places-poi-residual.md).

Bootstrap takes that same source lock only while establishing a repeatable-read snapshot and its capture boundary, then releases it before exporting rows or contacting the target. The source pool needs at least two connections. It refuses an occupied destination scope and unsupported rows. Baseline export is bounded to 100,000 rows; transactions exceeding 1,000 changes or 1 MiB stop capture instead of being truncated.

The installer needs DDL privileges on `public.user_favorites`, `public.user_likes` and `public.places` and permission to create `migration_capture`. All functions run as the caller, not SECURITY DEFINER. The application role needs schema usage, function execution, SELECT on control/scope/transactions/receipts, UPDATE on control/transactions, INSERT on transactions/changes/receipts, and usage on the changes identity sequence, in addition to its existing business-table permissions. Review explicit grants for the actual deployment roles; the installer does not guess role names or grant public access.

The runtime source account needs only CONNECT, schema usage, and SELECT on capture tables and the source interaction/catalogue tables. It makes no source business writes and performs no source acknowledgements. Source history and receipts are retained; no automatic retention/pruning is implemented. Target capture progress advances only after the full envelope is durably stored. A crash between enqueue and progress update safely repeats the immutable envelope.

The serialization lock adds contention to source interaction and catalogue writes, including writes outside the scope. Rehearse latency and source storage growth before enabling it. This adapter supplies durable command acceptance and detects gaps; it is not a generic database fallback, and it is not evidence that production capacity, privileges, retention, or failover have been verified.


Source IDs use character columns. Scope matching compares textual representations
without casting arbitrary world-name IDs to UUID. Noncanonical scoped IDs fail
validation. The source capture metadata still uses canonical UUID keys.

This installer accepts only `places_hybrid_commands_v1`, control version 4. It does not
upgrade, relabel or erase earlier capture histories. Existing expanded rehearsals
must stop and retain their evidence while operators prepare a new reviewed clean
baseline. Source removal and target cleanup are explicit operator migrations.
