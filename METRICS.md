# Measured query workloads

## Social notification polling and oversized-event expiry

The explicit `rpc::tests::fanout::notification_and_spill_queries_use_bounded_index_paths`
PostgreSQL test inserts 5,000 community rows with 10 tracked notification generations
and 5,000 temporary payload rows with 10 expired entries inside a rolled-back transaction.
The statements run `EXPLAIN (ANALYZE, BUFFERS)` after `ANALYZE`.

A local run on 2026-11-03 selected `communities_voice_notifications_idx` through a
bitmap index/heap scan followed by PK ordering: 0.069 ms execution, 14 matching rows
(the 10 fixture generations plus concurrent test rows), four shared-buffer hits.
The ordered expiry selection used `event_payloads_expiry_idx`: 0.016 ms execution,
10 rows, 13 shared-buffer hits including row locks. Both statements cap returned
work at 64 rows. The test checks that these indexes remain usable.

These small warm-cache plans establish access paths, not production throughput or
capacity for the 512 MiB payload admission bound. Production active-room counts,
large-event frequency, cold-cache behavior, index write cost and maximum-payload
memory/latency remain unmeasured.
