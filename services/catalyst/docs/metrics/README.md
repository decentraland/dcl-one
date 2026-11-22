# Metrics

`catalyst serve` keeps the Prometheus series of the content server and lamb2,
under their names, labels and buckets, so that existing dashboards keep
working. Each area page maps every legacy series to what the node emits:

- [Content server](content.md);
- [Sync](sync.md): the snapshots fetcher and the deployments-to-SQS series;
- [Lambdas](lambdas.md): lamb2.

## The `/metrics` listener

The process serves every series from one Prometheus exporter, on a listener
separate from the API:

| Setting | Default | Meaning |
| --- | --- | --- |
| `SERVE_METRICS_BIND` (`--serve-metrics-bind`) | `127.0.0.1:5006` | Address of the exporter's HTTP listener. It must be a loopback address; startup fails otherwise. |

`GET http://<bind>/metrics` answers the Prometheus text format with no
authentication. The API port (`HTTP_SERVER_PORT`, 5210) does not serve these
series, and the gateway answers 404 for `/metrics`
([gateway](../../../../infra/gateway/README.md)).

The content server and lamb2 each had their own endpoint. Here both are in the
one exporter, together with every other area's series.

## Scraping in production

The listener is reachable only from the network namespace of the `catalyst
serve` process. Scrape it from inside that namespace, without publishing the
port:

- run the Prometheus agent (or a compatible collector, such as
  `prometheus --agent` with `remote_write`) as a sidecar sharing the API
  container's network (`--network container:<apis>`, or a sidecar in the same
  Kubernetes pod), with a static target `127.0.0.1:5006`;
- or run the node with host networking and scrape `127.0.0.1:5006` from an
  agent on the host.

```yaml
scrape_configs:
  - job_name: catalyst
    static_configs:
      - targets: ["127.0.0.1:5006"]
```

The previous deployments exposed `/content_metrics` and `/lamb2_metrics`
through their nginx, behind basic auth
([catalyst-owner `docs/METRICS.md`](https://github.com/decentraland/catalyst-owner/blob/1866250dcff73a095ccba757d869766ea254bda9/docs/METRICS.md),
[`local/nginx/include/metrics.conf`](https://github.com/decentraland/catalyst-owner/blob/1866250dcff73a095ccba757d869766ea254bda9/local/nginx/include/metrics.conf)).
The gateway has no equivalent route: it reaches the APIs on their API port,
and the exporter only accepts loopback connections. A Prometheus that must
scrape from outside the node needs a proxy in the node's network namespace
that adds authentication (basic auth or a bearer token) before forwarding to
`127.0.0.1:5006`. Never expose the listener without authentication.

Which of these a deployment uses is an open choice: a scraping sidecar in the
node's network namespace, or an authenticating proxy there for an outside
Prometheus. Neither is provided by this repository yet.

### Dashboards that select a job

A scrape of the one endpoint gives every series the same `job` and `instance`.
A query that told the two services apart by a job selector needs that
selector removed or changed. Three families were emitted by both services:

- the per-route HTTP series, whose `handler` values are distinct except for
  `/status` (the content server's and lamb2's status routes now add up in one
  series);
- the subgraph series, now one set for the whole process;
- `wkc_logger_logs_total`, now one set for the whole process.

## Per-route HTTP series

The content and lambdas routes emit the series of `@dcl/http-server` (2.4.0 in
the content server, 2.1.0 in lamb2), from one middleware
(`shared::http_metrics::observe`) applied as the outermost layer of the
routers serving those routes: `content::build_router`, `shop::market::router`
and `land_authz::routes::router`.

| Series | Type | Labels | Buckets |
| --- | --- | --- | --- |
| `http_requests_total` | counter | `method`, `handler`, `code` | none |
| `http_request_duration_seconds` | histogram | `method`, `handler`, `code` | 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1, 2.5, 5, 10 |
| `http_request_size_bytes` | histogram | `method`, `handler`, `code` | the same (prom-client's defaults, which the legacy declarations kept) |

- `handler` is the route template, never the request path, in the
  services' syntax: `/contents/:hashId`, `/users/:address/wearables`.
  The `/content` and `/lambdas` prefixes are removed, as the old proxy
  removed them, so `/contents/{hashId}` and `/content/contents/{hashId}`
  share the label `/contents/:hashId`.
- A few lambdas paths were served by another route in the old deployment and
  take that route's label: `/lambdas/collections` is `/nfts/collections`, and
  `/lambdas/collections/contents/{pointer}/{image,thumbnail}` are the content
  server's `/queries/items/:pointer/...`. Both erc721 routes are
  `/queries/erc721/:chainId/:contract/:option/:emission?`.
- A request no route matched has an empty `handler`: an unknown path under
  `/content/` or `/lambdas/`. Unknown paths elsewhere are not counted. The
  old proxy answered some unknown `/lambdas/` paths itself, so lamb2 never
  counted those.
- A request whose path matches a route but whose method does not is counted
  under that route's template; the legacy services counted it with an empty
  `handler`.
- `method` is the request method, or `other` for a non-standard one. `code`
  is the response status.
- The duration ends when the handler returns the response, before a streamed
  body is sent. The size is the request's `Content-Length`, or 0.
- Routes the node adds (such as `/content/forwarding` or
  `/lambdas/land-authz/status`) are counted under their own templates. The
  trades reads the Shop router serves at `/v1/trades` are not counted.

## Alerts

The alert rules live outside this repository, with the deployment's
Prometheus or Grafana configuration, and must be checked there against the
area pages. A typical availability alert reads these legacy series:

| Concern | Legacy series | Source |
| --- | --- | --- |
| Deployment errors | `dcl_deployments_endpoint_counter{kind}` (`error`, `validation_error`), `dcl_deployment_time{failed}` | content server |
| Sync state | `dcl_content_server_sync_state`, `dcl_bootstrapping_servers`, `dcl_syncing_servers` | content server, snapshots fetcher |
| Sync stream failures | `dcl_deployments_stream_failure_count`, `dcl_content_download_errors_total` | snapshots fetcher |
| Failed deployments | `dcl_content_server_failed_deployments`, `entity_deployment_failure`, `entity_download_failure` | content server, deployments-to-SQS |
| HTTP 5xx rate | `http_requests_total{code=~"5.."}` by `handler` | content server, lamb2 |

Whether each is emitted, replaced or not applicable is stated in
[content](content.md), [sync](sync.md) and [lambdas](lambdas.md). A rule that
also selects `job` needs the change described in
[Dashboards that select a job](#dashboards-that-select-a-job).
