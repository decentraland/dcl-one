# Worlds partial uploads own one database session

- Status: Accepted
- Date: 2026-11-16

## Context

Batches for one signed entity can arrive on several replicas. Staging must remain
resumable across request failures, and the shared PostgreSQL pool supports a
single connection. A process mutex cannot order batches arriving on different
replicas. A session lock held on an additional connection would prevent a
single-connection pool from executing the batch's own SQL.

## Decision

Each partial batch owns one connection from the shared pool. A Worlds entity
session advisory lock orders its first pending-upload and replay reads, staging
steps, validation and committed publication. All SQL in that guarded path uses
the owned connection. Failed nonblocking lock attempts return their connection
before waiting. Different entities use independent advisory keys.

Upsert, reservation, stored-file progress and publication keep their separate
transactions and commits. Blob persistence remains outside those transactions.
The publication transaction retains its shared store lock and final expiry,
authorization and quota checks. Committed staging rows and completion receipts
remain the durable recovery authority.

Connection ownership precedes the name deny-list refresh mutex for both batch
and ordinary readers. Otherwise an ordinary reader could own that mutex while
waiting for the batch's sole pooled connection.

The guard owns the connection until unlock is confirmed. Cancellation or an
uncertain lock/unlock closes the session instead of returning it to the pool.
Subsequent guarded SQL uses the same session, so transport loss cannot silently
continue on a replacement connection without the entity lock.

After a successful publication commits, the batch releases its session before
running the existing postcommit effects. Owner-cache update, synchronous access
revocation, deployment logging and metrics, quota scheduling and forwarding
notification retain their order before the originating HTTP response.

Expired-upload cleanup retains its exclusive store lock and existing expiry
checks. It does not acquire the entity session lock: cleanup may reclaim an
expired upload between independently committed staging steps. Those steps keep
their disappearance, expiry and completion replay handling.

## Consequences

A partial batch occupies one pooled session during external validation and blob
I/O, while holding no batch-wide transaction. This uses the existing upload
admission limit and pool; there is no additional connection budget or response
status. Request cancellation releases pool capacity by closing uncertain
sessions. Runtime verification covers replica ordering, single-connection pools,
durable progress, expiry and cancellation.
