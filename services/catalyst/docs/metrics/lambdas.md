# Lambdas metrics

`catalyst serve` answers the `/lambdas` routes that lamb2 served. This page maps
every series lamb2 declared in
[`src/metrics.ts`](https://github.com/decentraland/lamb2/blob/02a37f5d88fb895339c20e84c2470be68cdf3f57/src/metrics.ts)
at `02a37f5d` to what the node emits. The families are declared in their owning
crates and composed into serve's registry; endpoint and scraping are in the
[metrics overview](README.md).

## lamb2's own series

| Legacy series | Type and labels | Status | Observed at | Notes |
| --- | --- | --- | --- | --- |
| `dcl_lamb2_server_build_info` | gauge `commitHash` | emitted | `shop::market::lambdas_metrics::record_build_info`, called from `serve` at startup | Set to 1. `commitHash` is `COMMIT_HASH`, else the build's revision, else empty. lamb2 used `COMMIT_HASH`, else empty. |
| `profiles_counter` | counter `pathname`, `ids` | not applicable | none | lamb2 declares it but never increments it, so it never had a sample. Its `ids` label would also be unbounded. |
| `tpw_provider_fetch_assets_duration_seconds` | histogram `id` | not applicable | none | Declared, never observed by lamb2. |
| `cache_warmer_collections_warmed_total` | counter `collection` | not applicable | none | lamb2 declares the six `cache_warmer_*` series but its warmer records none of them. The node has no third-party collection cache warmer. |
| `cache_warmer_duration_seconds` | histogram `collection` | not applicable | none | As above. |
| `cache_warmer_errors_total` | counter `collection` | not applicable | none | As above. |
| `cache_warmer_fatal_errors_total` | counter | not applicable | none | As above. lamb2 rendered a constant 0, as prom-client does for an unlabelled series. |
| `cache_warmer_total_duration_seconds` | histogram | not applicable | none | As above. lamb2 rendered empty buckets, as prom-client does for an unlabelled series. |
| `cache_warmer_last_warmup_timestamp` | gauge | not applicable | none | As above. lamb2 rendered a constant 0, as prom-client does for an unlabelled series. |
| `marketplace_api_fetches` | gauge `state` (`running`, `queued`) | not applicable | none | lamb2 bounded its inventory fetches to the marketplace API in a bulkhead. The node reads owned items from its `nft` index and names from the marketplace subgraph, with no bulkhead or queue to measure. |
| `marketplace_api_fetches_shed_total` | counter `operation` | not applicable | none | Counted fetches the bulkhead refused. The node has no bulkhead and refuses none. |
| `elements_cache_reads_total` | counter `result` (`hit`, `stale`, `miss`) | replacement `dcl_lambdas_owner_memo_reads_total{result}` | `shop::market::user_items`, the `/lambdas/users/{address}/names` route when it reads the subgraph | lamb2 counted every read of its shared owned-elements cache, including the three reads behind each profile. The node has no such cache. Wearables and emotes come from the `nft` index, profile ownership is a batched check, and LAND permissions come from the land-authz index, none of them memoized. Names read from the marketplace subgraph are memoized per owner for 30 s (`chain_index::graph`). The replacement counts those reads: `hit` when a fresh entry answers, and `miss` when the read fetches from the subgraph or joins a fetch already in flight. It exists only where the marketplace subgraph is the node's NAME source: a marketplace subgraph URL is set, and the node is not on mainnet with the `nft` index on. A mainnet node with the `nft` index reads names from that index and never emits it. It has no `stale` value, because the memo never serves an expired entry while refreshing it. It is not emitted under the legacy name, because it covers far fewer reads and would read as a drop in traffic. |
| `elements_cache_background_refresh_total` | counter `outcome` | not applicable | none | The node refreshes no entry in the background. A failed names read answers the last list fetched in the past 120 s, which is a fallback, not a refresh. |

## Series lamb2 inherited

lamb2 merged three packages' declarations into its registry.

| Legacy series | Type and labels | Status | Observed at | Notes |
| --- | --- | --- | --- | --- |
| `http_requests_total` | counter `method`, `handler`, `code` | replaced by `dcl_http_requests_total{component,route,status_code,method}` | the shared HTTP layer, installed with `component(...)` in `src/serve_routes.rs` | `@dcl/http-server` 2.1.0. See [HTTP requests](README.md#http-requests) for the label mapping. |
| `http_request_duration_seconds` | histogram `method`, `handler`, `code` | replaced by `dcl_http_request_duration_seconds` | as above | The shared buckets add a 30 s bound to prom-client's defaults. |
| `http_request_size_bytes` | histogram `method`, `handler`, `code` | replaced by `dcl_http_request_declared_size_bytes` | as above | Observed only for requests with a valid `Content-Length`, in byte-sized buckets. |
| `wkc_logger_logs_total` | counter `logger`, `level` | not applicable | none | `@well-known-components/logger` 3.1.3. In serve this family belongs to Social's RPC, which counts its own logger names. A process-wide count of `tracing` events under the same name would mix in every other area's logs and double-count Social's. |
| `subgraph_ok_total` | counter `url` | emitted | `chain_index::graph::graph_query_classified`, the one function every subgraph query goes through | `@dcl/thegraph-component` 0.1.2. Counts each valid answer. |
| `subgraph_errors_total` | counter `url`, `kind` | emitted | as above | Counts each failed attempt (`kind="timeout"` when the request timed out, `unknown` otherwise), and each answer with an `errors` key, even `[]` (`invalid_block`, `syntax_error` or `unknown`, as the old component classified it), or whose `data` is missing, `null` or empty (`unknown`). Such an answer fails the query, as it did in the old component. The node does not retry GraphQL errors; the old component did. An attempt its caller stops waiting for counts no error. |
| `subgraph_query_duration_seconds` | histogram `url` | emitted | as above | Each attempt's duration, including an attempt its caller stops waiting for, buckets 0.1, 0.3, 0.5, 1, 2, 5, 10, 20, 30, 60. |

The `subgraph_*` series also cover serve's other subgraph readers (builder,
rentals, social), as the old services' registries covered theirs. The
`catalyst index` process declares and emits them too, for its mirrors and
readers, on its own `/metrics`.

`url` is the configured subgraph URL, except that the path segment after
`/api/`, where a gateway URL carries its API key, reads `[redacted]`:
`/api/<key>/subgraphs/id/<id>` becomes `/api/[redacted]/subgraphs/id/<id>`.
The old component exported the key, so a keyed URL's label value differs from
the legacy one.
