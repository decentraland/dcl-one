# Metrics

`catalyst serve` keeps the content server's and lamb2's Prometheus series
under their names, labels and buckets, unless dcl-one already measures the
same thing under another name. Each area page maps every legacy series to
what the node emits:

- [Content server](content.md);
- [Sync](sync.md): the snapshots fetcher and the deployments-to-SQS series;
- [Lambdas](lambdas.md): lamb2.

## Endpoint and scraping

Each process serves all of its series at one `GET /metrics` on its operator
listener. Prometheus scrapes each process directly. The
[shared service metrics ADR](../../../../docs/adr/2026-11-24-shared-service-metrics.md)
lists the addresses, the scrape configuration and how families are declared.

The content server and lamb2 were scraped as two jobs, each on its own path.
In dcl-one, both families come from the serve replica that handled the work,
under the job that scrapes serve (`catalyst-serve` in the ADR's example). A
dashboard or alert query that selects the old content or lamb2 job needs that
selector changed to the serve job.

## HTTP requests

Content and lambdas requests are counted by the shared HTTP families, not by
the `@dcl/http-server` series the content server and lamb2 exported
(`http_requests_total`, `http_request_duration_seconds`,
`http_request_size_bytes`). Serve does not emit those three for these routes.

| Legacy query | dcl-one query |
| --- | --- |
| `http_requests_total{handler, code, method}` | `dcl_http_requests_total{component, route, status_code, method}` |
| `http_request_duration_seconds` | `dcl_http_request_duration_seconds` (largest finite bucket 30 s) |
| `http_request_size_bytes` | `dcl_http_request_declared_size_bytes` (requests with a valid `Content-Length`) |

- `component` is `content` for the content routes, at the root and under
  `/content`, and for the `/lambdas` routes the content router serves
  (`/lambdas/about`, `/lambdas/explore/realms`,
  `/lambdas/collections/contents/...`). It is `shop` for the other
  `/lambdas` routes, and `land-authz` for the LAND permission and operator
  routes.
- `route` is the matched axum template as the node serves it, prefix
  included: the legacy `handler="/contents/:hashId"` is
  `route="/content/contents/{hashId}"` or `route="/contents/{hashId}"`, and
  lamb2's `handler="/users/:address/wearables"` is
  `route="/lambdas/users/{address}/wearables"`. A request no route matched
  is `route="unmatched"`.
- `status_code` is the response status, or `cancelled` when the handler was
  interrupted before its response headers.
- The content server's and lamb2's `/status` are now separate routes,
  `/content/status` (or `/status` at the root) and `/lambdas/status`.

For example, the legacy 5xx rate of content routes,
`sum(rate(http_requests_total{job="<content job>", code=~"5.."}[5m]))`, is
`sum(rate(dcl_http_requests_total{job="catalyst-serve", component="content", status_code=~"5.."}[5m]))`.

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
| HTTP 5xx rate | `http_requests_total{code=~"5.."}` by `handler`; now `dcl_http_requests_total{status_code=~"5.."}` by `component` and `route` | content server, lamb2 |

Whether each is emitted, replaced or not applicable is stated in
[content](content.md), [sync](sync.md) and [lambdas](lambdas.md).
