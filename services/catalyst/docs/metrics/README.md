# Metrics

`catalyst serve` keeps the Prometheus series of the content server and lamb2,
under their names, labels and buckets, so that existing dashboards keep
working. Each area page maps every legacy series to what the node emits:

- [Content server](content.md);
- [Sync](sync.md): the snapshots fetcher and the deployments-to-SQS series;
- [Lambdas](lambdas.md): lamb2.

## One endpoint for the whole process

`catalyst serve` records every series in one Prometheus exporter, and the
gateway publishes it as one endpoint, `/metrics`, behind basic auth. The
content server's `/content_metrics` and lamb2's `/lamb2_metrics` have no
equivalent: both families, and every other area's, are in this one endpoint.

The exporter is reachable in two ways:

| Path | Who can reach it | Authentication |
| --- | --- | --- |
| `http://<SERVE_METRICS_BIND>/metrics` (default `127.0.0.1:5006`) | the API process's own network namespace; the bind must be a loopback address | none |
| `GET /internal/metrics` on the API port (`HTTP_SERVER_PORT`, 5210) | a loopback peer, or a peer in `HTTP_TRUSTED_PROXY_CIDRS` (the gateway's network); 404 to anyone else | none; the gateway never forwards this path |
| `GET /metrics` on the gateway | Prometheus, from anywhere the gateway is reachable | basic auth, only when configured |

## Configuring the credentials

1. Create an htpasswd file, for example `htpasswd -B -c metrics.htpasswd <user>`.
   Keep it with the deployment's other secrets, never in the repository.
2. Mount it read-only into the gateway container and set
   `GATEWAY_METRICS_HTPASSWD` to its path, for example
   `/etc/gateway/metrics.htpasswd`.
3. List the gateway's container network in the API's
   `HTTP_TRUSTED_PROXY_CIDRS`, which a gateway deployment already needs for
   client addresses ([gateway](../../../../infra/gateway/README.md)).

Without `GATEWAY_METRICS_HTPASSWD`, the gateway answers 404 for `/metrics`:
nothing is public by default. With it, only the exact path `/metrics` opens;
`/metrics/...`, other spellings and `/internal/metrics` stay 404.

## Scraping

One Prometheus job scrapes the whole process:

```yaml
scrape_configs:
  - job_name: catalyst
    scheme: https
    metrics_path: /metrics
    basic_auth:
      username: <user>
      password_file: /etc/prometheus/catalyst-metrics-password
    static_configs:
      - targets: ["<node-domain>"]
```

An agent in the API's network namespace (a sidecar sharing the API
container's network, or the host with host networking) can instead scrape
`127.0.0.1:5006` directly, without the gateway.

The subgraph series carry the configured subgraph URL in their `url` label
([lambdas](lambdas.md)). A URL that embeds an API key is therefore visible to
anyone who can read the endpoint, which is why it is only served behind
authentication.

### Dashboards that select a job

The previous deployments scraped the content server and lamb2 as two jobs.
One endpoint gives every series the same `job` and `instance`, so a dashboard
or alert query that filtered on the old job (content vs lamb2) needs that
filter removed or changed to the new job. Three families were emitted by both
services:

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
