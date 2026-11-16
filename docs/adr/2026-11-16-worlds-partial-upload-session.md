# Worlds partial uploads own one database session

- Status: Accepted
- Date: 2026-11-16

## Context

Batches for one signed entity can arrive on several replicas. Staging must
remain resumable across request failures, and the shared PostgreSQL pool
supports a single connection. A process mutex cannot order batches arriving on
different replicas. A session lock held on an additional connection would
prevent a single-connection pool from executing the batch's own SQL.

## Decision

Each partial batch owns one connection from the shared pool. A Worlds entity
session advisory lock orders its first pending-upload and replay reads, staging
steps, validation and committed publication. All SQL in that guarded path uses
the owned connection. Before a checkout, the batch acquires a permit from the
shared CoreState's partial-session budget: `min(upload_limit, max(1, pool_max /
4))`, using the existing concurrent-upload limit and configured pool maximum.
This conservative share bounds partial-upload session occupancy while retaining
support for a single-connection pool. Failed nonblocking lock attempts return
both their connection and permit before waiting. Different entities use
independent advisory keys.

Upsert, reservation, stored-file progress and publication keep their separate
transactions and commits. Blob persistence remains outside those transactions.
The publication transaction retains its shared store lock and final expiry,
authorization and quota checks. Committed staging rows and completion receipts
remain the durable recovery authority.

Connection ownership precedes the name deny-list refresh mutex for both batch
and ordinary refreshers. An ordinary reader that finds the refresh mutex busy
returns its connection before waiting for that refresh and retrying its cache.
Otherwise an ordinary reader could own that mutex while waiting for the batch's
sole pooled connection.

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
I/O, while holding no batch-wide transaction. Partial-session ownership is
bounded by the per-process permit budget; ordinary publishes keep their short
per-query checkouts. Waiting for a permit uses the existing
deployment-processing request timeout and cancellation behavior, without
another response status or lock deadline. Throughput and latency under this
conservative capacity choice remain unmeasured. Request cancellation releases
pool capacity by closing uncertain sessions. Runtime verification covers
replica ordering, single-connection pools, durable progress, expiry and
cancellation.
