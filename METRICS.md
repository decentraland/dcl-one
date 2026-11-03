# Measured query workloads

## Social notification polling and oversized-event expiry

The explicit `rpc::tests::fanout::notification_and_spill_queries_use_bounded_index_paths`
PostgreSQL test uses its own temporary database and inserts 5,000 community rows
with 10 tracked notification generations and 5,000 temporary payload rows with 10
expired entries inside a rolled-back transaction.
The statements run `EXPLAIN (ANALYZE, BUFFERS)` after `ANALYZE`.

A local run on 2026-11-03 selected `communities_voice_notifications_idx` through a
bitmap index/heap scan followed by PK ordering: 0.041 ms execution, 10 matching rows,
three shared-buffer hits. The ordered expiry selection used
`event_payloads_expiry_idx`: 0.033 ms execution,
10 rows, 13 shared-buffer hits including row locks. Both statements cap returned
work at 64 rows. The test checks that these indexes remain usable.

These small warm-cache plans establish access paths, not production throughput or
capacity for the 512 MiB payload admission bound. Production active-room counts,
large-event frequency, cold-cache behavior, index write cost and maximum-payload
memory/latency remain unmeasured.

## Social mutation burst fanout

The real-NATS tests run an owned broker and two links, with 1,000 updates for one
local subscription. A targeted block burst delivered all 1,000 updates in order,
without origin loopback duplication, through awaited publisher admission and the
synchronous target callback. The community test uses PostgreSQL membership reads,
a pool of eight connections, and 100 interleaved messages on a control subject.

A local run on 2026-11-03 with the former 256-entry serial community queue delivered
331 of 1,000 updates; the 100 control messages arrived in 16.145 ms. With the current
1,000-entry queue and four concurrent deliveries, all 1,000 updates arrived; the
control messages arrived in 24.757 ms. This is one small warm-cache burst fixture,
not a sustained throughput, multi-community fairness or maximum-payload benchmark.
SQL concurrency is capped by the configured pool size; ingress beyond the queue
bound can still drop transient events and increments the delivery-drop counter.

The targeted burst harness records readiness, submission, broker observer counts
and both subscription queue lengths. In a 2026-11-03 full Social run it reached
readiness in 5.007 seconds, finished submission at 5.021 seconds and observed
all 1,000 frames at both broker links and both queues by 5.027 seconds. The same
run delivered all community updates and its 100 controls in 28.482 milliseconds.
A process snapshot near the run's start contained one Cargo process and no Rust
compiler process; it is not a continuous system-load measurement. An earlier
complete-suite targeted-burst timeout had no stage counts; its focused rerun
passed. The cause of that timeout remains unestablished. The harness retains its
10-second delivery deadline and exact count, ordering and duplicate assertions;
that deadline is not a protocol latency guarantee.

## Ordinary Social RPC writer

The source transport fixture executes the pinned TypeScript transport with a
controlled socket and virtual timers. Its 17 cases cover queue capacity,
backoff, drain callbacks, accepted-once handling, closure and explicit unsupported
configuration edges. A real socket test on 2026-11-03 preserved an 8 MiB profile
URL in a unary response, then stopped reading and observed transport retirement
after bounded retries of subsequent responses. That focused test completed in
1.06 seconds. These checks do not measure native uWS buffer behavior, kernel
capacity, sustained throughput or maximum-memory consumption. Accepted bytes are
charged through the existing writer's physical flush; pending-message storage
grows on demand up to the configured finite threshold.

## Private-call RPC UUID boundary

The private identifier replay sampled 128 sequential SQLx executions of the bound
scalar `SELECT $1::text::uuid` on the existing test pool. On local PostgreSQL 18.6,
over loopback with a warm statement cache and concurrent builds, one 2026-11-03
run measured p50 133 microseconds, p95 848 microseconds and maximum 5,001
microseconds. These timings include pool acquisition and the query round trip.
The sample adds no application-table reads or row locks. Hardware details, p99,
sustained throughput and production-load latency were not recorded. The RPC now
performs this extra query; the measurement does not claim source performance parity.

## Community ranking and compliance cache

A disposable PostgreSQL 18.6 run on 2026-11-03 used the schema and query text in
`services/catalyst/social/src/communities/ranking.rs` and `compliance.rs`,
including the cache expiry recheck. The fixture contained 100 communities, 100,000 memberships, 10,000 posts,
1,000 community places, 100,000 photos, 100,000 ended local event occurrences and
100,000 compliance results. These are single `EXPLAIN (ANALYZE, BUFFERS)` samples
from a developer machine during a concurrent build, with uncontrolled cache state.

| Statement | Fixture result | Execution time |
| --- | --- | ---: |
| Ranking facts for 100 communities | 100 rows | 100.389 ms |
| Empty pending-photo page, partial index | 0 of 100,000 | 0.045 ms |
| Same empty page, index scans disabled | 0 of 100,000 | 30.039 ms |
| Pending-photo page, partial index | 50 of 100,000 | 0.166 ms |
| Photo receipt and counter fanout | 100 communities | 11.098 ms |
| Ended local occurrence scan | capped at 100 of 100,000 | 538.459 ms |
| Fresh compliance result by primary key | 1 of 100,000 | 0.057 ms |
| Expired compliance deletion | capped at 1,000 of 100,000 | 85.488 ms |

The partial index avoids a full photo-history scan when no repair is due. Cache
reads use the primary key; bounded expiry selection uses the expiry index. The
occurrence scan and ranking aggregates still depend on the amount of local history
and membership data. Repeated latency percentiles, production distributions,
concurrent writer throughput, cold-cache performance, hardware characterization,
receipt growth and replica lag remain unmeasured. These samples establish query
paths and bounded output sizes, without establishing production capacity.

## Shared social registry profile cache

A disposable PostgreSQL 18.6 run on 2026-11-03 populated a temporary cache relation
with 100,000 synthetic rows and the canonical primary/expiry indexes. Single
`EXPLAIN (ANALYZE, BUFFERS)` samples during a concurrent build measured:

| Statement | Fixture result | Execution time |
| --- | --- | ---: |
| Fresh profile key set | Primary-key index scan | 5.919 ms |
| Bulk conflicting profile write | 1,000 keys | 9.200 ms |
| Expired selection and deletion | capped at 1,000 rows | 2.465 ms |

Expiry selection used the expiry index and deletion used tuple identifiers. The
reader always excludes expired rows independently of this sweep. These samples
show the selected access paths and bounded cleanup output; they do not establish
production latency, a sustained 1,000-key-per-second reclamation rate or storage
capacity. Production key distributions, concurrent writers, repeated percentiles,
cold-cache costs and the existing source-cache cutover remain unmeasured.
