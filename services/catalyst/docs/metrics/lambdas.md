# Lambdas metrics

`catalyst serve` answers the `/lambdas` routes that lamb2 served. This page maps
every series lamb2 declared in
[`src/metrics.ts`](https://github.com/decentraland/lamb2/blob/02a37f5d88fb895339c20e84c2470be68cdf3f57/src/metrics.ts)
at `02a37f5d` to what the node emits. Scraping is described in the
[metrics overview](README.md).

## lamb2's own series

| Legacy series | Type and labels | Status | Observed at | Notes |
| --- | --- | --- | --- | --- |
| `dcl_lamb2_server_build_info` | gauge `commitHash` | emitted | `shop::market::lambdas_http::register_series`, called from `serve` at startup | Set to 1. `commitHash` is `COMMIT_HASH`, else the build's revision, else empty. lamb2 used `COMMIT_HASH`, else empty. |
| `profiles_counter` | counter `pathname`, `ids` | not applicable | none | lamb2 declares it but never increments it, so it never had a sample. Its `ids` label would also be unbounded. |
| `tpw_provider_fetch_assets_duration_seconds` | histogram `id` | not applicable | none | Declared, never observed by lamb2. |
| `cache_warmer_collections_warmed_total` | counter `collection` | not applicable | none | lamb2 declares the six `cache_warmer_*` series but its warmer records none of them. The node has no third-party collection cache warmer. |
| `cache_warmer_duration_seconds` | histogram `collection` | not applicable | none | As above. |
| `cache_warmer_errors_total` | counter `collection` | not applicable | none | As above. |
| `cache_warmer_fatal_errors_total` | counter | not applicable | none | As above. lamb2 rendered a constant 0, as prom-client does for an unlabelled series. |
| `cache_warmer_total_duration_seconds` | histogram | not applicable | none | As above. lamb2 rendered empty buckets, as prom-client does for an unlabelled series. |
| `cache_warmer_last_warmup_timestamp` | gauge | not applicable | none | As above. lamb2 rendered a constant 0, as prom-client does for an unlabelled series. |
| `marketplace_api_fetches` | gauge `state` (`running`, `queued`) | not applicable | none | lamb2 bounded its inventory fetches to the marketplace API in a bulkhead. The node reads owned items from the collections and marketplace subgraphs (`chain_index::graph`), without a bulkhead or a queue to measure. |
| `marketplace_api_fetches_shed_total` | counter `operation` | not applicable | none | Counted fetches the bulkhead refused. The node has no bulkhead and refuses none. |
| `elements_cache_reads_total` | counter `result` (`hit`, `stale`, `miss`) | not emitted | none | lamb2 counted every read of its shared owned-elements cache, including the three reads behind each profile. The node has no such cache: owned copies and names are memoized per owner for 30 s (`chain_index::graph`), profile ownership is a batched subgraph check, and LAND permissions come from the land-authz index. Counting the memo alone under this name would read as a drop in traffic, and it never serves stale entries while it refreshes them. No replacement series counts the memo's hits and misses yet. |
| `elements_cache_background_refresh_total` | counter `outcome` | not applicable | none | The node refreshes no entry in the background. A failed names read answers the last list fetched in the past 120 s, which is a fallback, not a refresh. |

## Series lamb2 inherited

lamb2 merged three packages' declarations into its registry.

| Legacy series | Type and labels | Status | Observed at | Notes |
| --- | --- | --- | --- | --- |
| `http_requests_total` | counter `method`, `handler`, `code` | emitted | `shared::http_metrics::observe`, the outermost layer of `shop::market::router`, `land_authz::routes::router` and `content::build_router` | `@dcl/http-server` 2.1.0. See [Per-route HTTP series](README.md#per-route-http-series) for the handler labels. |
| `http_request_duration_seconds` | histogram `method`, `handler`, `code` | emitted | as above | prom-client's default buckets. |
| `http_request_size_bytes` | histogram `method`, `handler`, `code` | emitted | as above | The request's `Content-Length`, or 0. prom-client's default buckets, as lamb2 used. |
| `wkc_logger_logs_total` | counter `logger`, `level` | not emitted | none | `@well-known-components/logger`. The node logs through `tracing`, whose targets do not match lamb2's logger names. |
| `subgraph_ok_total`, `subgraph_errors_total`, `subgraph_query_duration_seconds` | counter `url`; counter `url`, `kind`; histogram `url` | not emitted | none | `@dcl/thegraph-component` 0.1.2. Subgraph queries run in `chain_index::graph` and are not measured yet. |
