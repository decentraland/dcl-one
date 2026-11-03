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

## Referral recent-invitation read

A 2026-11-03 PostgreSQL 18.6 TEMP fixture held 100,000 referral rows with the current
`(referrer,status)` index. The new post-insert query orders one referrer's history
by creation time and returns at most two rows. Single local
`EXPLAIN (ANALYZE, BUFFERS)` samples used a bitmap index/heap scan and top-N heapsort:
90 matching rows took 0.086 ms; a skewed referrer with 10,000 matching rows took
0.870 ms. These are query execution observations from a developer machine during
a build. They do not include HTTP/Slack time, prove sustained throughput or bound
scan work as one referrer's history grows. No additional index is inferred from
these samples.

The community SNS replay on the same date executed 18 source broadcaster cases,
two SNS adapter result cases and twelve source event creators. The local target
used its real membership SQL and the official AWS SDK with a local HTTP stub.
For a rename of 1,101 members, both sides fetched twelve membership pages,
created twelve recipient messages and made two PublishBatch requests (ten and
two entries). A 205-member deletion excluded its owner and emitted three
recipient messages. Partial result IDs and full failed chunks were compared;
failed broadcasts did not undo HTTP ban or voice START. This establishes the
sampled audience, batching and request shapes. SNS service latency, retry timing,
concurrent membership mutation across pages and sustained throughput were not
measured.

## Social HTTP body boundary

The 2026-11-03 body replay used social-service-ea at
`b6b2cefefdf727b2ed47ddb887e77d226a41eea6`, Node 26.12.0 and its installed
`@dcl/http-server` 2.3.0. It compared 18 startup inputs and 44 HTTP cases: 39 with
a 64-byte cap and five multipart cases with a 2 MiB cap. The target used real
Hyper, the actual community and RPC HTTP routes, the shared unread-body drain
and the outer trailing-slash normalizer. The replay compared status, complete
body and every application header; date and transport framing/hop headers were
excluded. Declared oversize and consumed-stream overflow explicitly checked the
close header. A declared oversized request received its response without an
upload body. Another domain accepted 65 bytes behind the same outer layers.

The larger-cap fixture was valid native multipart with 1,050,055 wire bytes,
including a 1,024-byte thumbnail and an unused padding field. Declared, chunked
and header-only variants all reached the independent multipart 400. Thumbnail
limits were checked separately while the total stayed below 1 MiB. This replay
does not measure RSS, concurrent upload capacity, throughput, or unread chunked
connection-close timing. The latter remains a separate source/target audit.

## Private voice pending and retained media

A 2026-11-03 owned PostgreSQL fixture contained 10,512 media calls and 21,024
participants, including 512 historical calls for one wallet. Each media
participant was past its initial-connection TTL; no pending calls remained.
The target INSERT statement was extracted from the Rust write path. Fifty
local starts per wallet measured BEGIN, explicit READ COMMITTED, sorted
participant advisory locks, INSERT and COMMIT; cleanup was excluded.

| Wallet history | Median start | Local p95 | Final INSERT plan execution |
| --- | ---: | ---: | ---: |
| One retained call | 0.419 ms | 0.663 ms | 0.026 ms |
| 512 retained calls | 1.798 ms | 2.367 ms | 1.755 ms |

Both plans used the nonunique caller/callee indexes. The small history used the
participant primary key; the skewed case used a hash anti-join against the
participant set. Repeated insert/delete cleanup left dead index entries during
the sample. Rust compilation ran concurrently and cache state was uncontrolled.
These samples establish the access paths and local command cost; they do not
compare source performance or establish production capacity, contention latency,
cold-cache behavior or a service-level bound. Explicit isolation adds one SQL
command to pending creation so waiting writers observe the prior winner even
under a role whose default isolation is REPEATABLE READ.

## Community voice read separation

A 2026-11-03 PostgreSQL 18.6 TEMP fixture held 100,000 synthetic voice-presence
rows across 10,000 communities with the current `(community_id,address)` primary
key. Single local `EXPLAIN (ANALYZE, BUFFERS)` samples during a concurrent build
measured the former all-community status aggregation at 21.030 ms (100,000 rows
scanned), the existing comms bulk query for 50 community IDs at 0.999 ms (500 rows
through bitmap primary-key/heap scans), and the all-active-room query at
17.344 ms (100,000 rows scanned). Ordinary community pages now use the bulk
query after core SQL; active filters still require the all-room query.

These are individual query observations on a temporary relation with matching
columns and key, not production latency or a sustained-throughput measurement.
They exclude HTTP, profile enrichment and queueing, and do not establish cold
cache behavior, repeated percentiles or capacity under concurrent presence
writes. No index or schema change was added.
