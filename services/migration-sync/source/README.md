# Places outbox capture adapter

This opt-in pilot captures catalogue rows, favorites and likes/dislikes for an immutable list of Genesis place UUIDs. Install only with the explicit operator command after reviewing the source DDL and role grants. It is not automatically installed when the worker starts. Bootstrap imports the scoped source catalogue and refuses conflicting native target ownership. Shop, treasury and credits are excluded.

`places_outbox_v3` is a transactional outbox, not logical WAL decoding. Its monotonically committed transaction ordinals use the envelope's hexadecimal cursor representation; they must never be compared to PostgreSQL WAL LSNs. Source generation and adapter identity fence distinct capture histories. A restored/replaced source must receive a new reviewed generation and baseline before capture resumes; restoring the same generation and reusing positions cannot be treated as a safe failover.

A source-local BEFORE STATEMENT trigger serializes catalogue, favorite and like mutations under the same lock with a transaction advisory lock. Relevant rows and accepted command receipts share a transactional counter and commit atomically. Rollbacks consume no committed position. Internal SQL writes and deletes are captured even when they bypass HTTP. TRUNCATE is rejected because it cannot preserve this snapshot/capture contract. Disabling triggers, replacing the table, changing the capture schema, or bypassing triggers invalidates the pilot and requires stopping capture and a reviewed baseline.

Bootstrap takes that same source lock only while establishing a repeatable-read snapshot and its capture boundary, then releases it before exporting rows or contacting the target. The source pool needs at least two connections. It refuses an occupied destination scope and unsupported rows. Baseline export is bounded to 100,000 combined favorite/like rows; transactions over the envelope limit stop capture instead of being truncated.

The installer needs DDL privileges on `public.places`, `public.user_favorites` and `public.user_likes`, and permission to create `migration_capture`. All functions run as the caller, not SECURITY DEFINER. The application role needs schema usage, function execution, SELECT on control/scope/transactions/receipts, UPDATE on control/transactions, INSERT on transactions/changes/receipts, and usage on the changes identity sequence, in addition to its existing business-table permissions. Review explicit grants for the actual deployment roles; the installer does not guess role names or grant public access.

The runtime source account needs only CONNECT, schema usage, and SELECT on capture tables and the source favorite/like/catalogue tables. It makes no source business writes and performs no source acknowledgements. Source history and receipts are retained; no automatic retention/pruning is implemented. Target capture progress advances only after the full envelope is durably stored. A crash between enqueue and progress update safely repeats the immutable envelope.

The serialization lock adds contention to all source catalogue, favorite and like writes, including writes outside the scope. Rehearse latency and source storage growth before enabling it. This adapter is a bounded fallback for environments where logical CDC prerequisites are not established; it is not evidence that production capacity, privileges, retention, or failover have been verified.

Version 3 includes catalogue rows and `user_likes`, including the boolean vote,
activity, creation time and update time. A like-to-dislike update is a replacement
fact; deletion removes the fact. All three tables share a transaction cursor, so
mixed source transactions become one atomic target application. Target counts are
rebuilt from facts; source cached counts are not copied.

Installation and polling reject version 1/2 sources instead of silently claiming
catalogue coverage. Stop older streams and prepare a reviewed version 3 source
installation/new clean destination baseline. The installer does not alter an
existing capture history, reset generations, or silently upgrade triggers. Apply
the destination schema migration permitting version 3 before bootstrapping.
Existing target favorites **or likes** in the selected scope reject bootstrap,
preventing a mixed-time baseline. Catalogue rows may be absent or batch-imported,
but native content/local-world/moderation/ranking ownership blocks replacement.

No client operation header, gateway dual routing or source application change is required for source-only catalogue/interaction synchronization. Optional favorite command receipts remain supported; likes have no native duplicate command lane. A transaction that mixes a receipt-bearing favorite command with like or catalogue changes is rejected by the classifier rather than silently dropping its extra effects.

Catalogue projection excludes interaction counters and generated full-text fields,
normalizes timestamp serialization to UTC, and captures the effective source row.
Source category job updates therefore synchronize without a target job copy.
Normal target services remain disabled: catalogue disable/delete triggers retain
removal intents, which need explicit reconciliation before effect ownership handover.
Versions 1 and 2 lack catalogue capture and require a new reviewed baseline; the
installer and reader refuse to silently upgrade them.
