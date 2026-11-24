# Shared service metrics

- Status: Accepted
- Date: 2026-11-24

## Context

Before this change, Catalyst domain crates recorded into the Rust metrics facade. Serve, Pulse and the message router configured exposition independently. The standalone index worker enabled its listener and lag monitor only when INDEX_METRICS_BIND was set. HTTP instrumentation covered selected services, and WebSocket implementations measured different stages of delivery.

## Decision

Use a shared Rust library, dcl-metrics, for process recorder setup, HTTP observations, WebSocket observations and service metric declarations. Each executable initializes its recorder before starting workers. Each replica serves Prometheus exposition at exactly one address, `GET /metrics` on its operator listener. Service crates own domain metrics and record into their host process. Preserve domain metric names, units and histogram buckets. The HTTP migration below lists the changes for existing consumers. Use the shared HTTP families for request measurements across components.

Count an HTTP request once, through response headers, including an upgrade handshake. Track established WebSocket connections through closure or cancellation. Count messages at an identified transport boundary and distinguish send failures from successful writes. Use bounded labels and matched route templates. Process counters accumulate for the lifetime of the process.

## Consequences

Prometheus discovers individual processes. Domain libraries need no additional listeners. Instance identity belongs to target discovery. Each index process starts its listener and lag monitor by default. Allocate a distinct loopback socket for each index process sharing a network namespace. A bind conflict prevents index startup. The monitor checks chain heads once per minute and shares a probe across projections using the same RPC endpoint in that process. Compare parsed scrape output and exercise success, failure and cancellation paths when changing instrumentation.

## Scrape targets

A scrape reports observations made by one running process. For example, a Builder request and an ordinary Social WebSocket handled by the same serve replica appear together on that replica's port 5006. An index job handled by another process appears on that process's port 5008. Scaling serve to three replicas gives Prometheus three serve targets.

| Role | Address setting | Default target |
| --- | --- | --- |
| `catalyst serve` | `SERVE_METRICS_BIND` | `127.0.0.1:5006/metrics` |
| `catalyst pulse` | `PULSE_METRICS_BIND` | `127.0.0.1:5005/metrics` |
| `catalyst-message-router` | `MESSAGE_ROUTER_METRICS_BIND` | `127.0.0.1:5007/metrics` |
| `catalyst index` | `INDEX_METRICS_BIND` | `127.0.0.1:5008/metrics` |

Serve and index enforce a loopback bind. Pulse and the message router default to loopback and also accept other bind addresses. For those roles, restrict access to the configured socket at deployment. The message router also serves its health checks on that listener.

For a collector sharing the network namespace of one instance of each role, the following Prometheus configuration uses the default path `/metrics`. In a fleet, discover each replica and preserve its identity in the target labels. Local development stacks can assign different ports.

```yaml
scrape_configs:
  - job_name: catalyst-serve
    static_configs:
      - targets: ["127.0.0.1:5006"]
  - job_name: catalyst-pulse
    static_configs:
      - targets: ["127.0.0.1:5005"]
  - job_name: catalyst-message-router
    static_configs:
      - targets: ["127.0.0.1:5007"]
  - job_name: catalyst-index
    static_configs:
      - targets: ["127.0.0.1:5008"]
```

## Collector and dashboard migration

Before deploying, inventory queries and alerts using the old Stats, connector or Camera Reel scrape jobs. Check deployment-managed Prometheus rules and Grafana dashboards as well as repository configuration. The index alerts in `infra/monitoring/index.rules.yml` retain their `catalyst-index` job and metric names.

Stage the private scrape targets in service discovery before the rollout. As each replica is upgraded, verify its target is up and a known HTTP request increments the new series. Switch its dashboards and alerts to the new job and remove its application scrape targets in the same rollout. The upgraded replica immediately stops serving `/social-rpc/metrics`, `/archipelago-ea-stats/metrics`, `/archipelago-ea-ws-connector/metrics`, `/camera-reel-service/metrics` and configured aliases.

For Stats, connector and Camera Reel, migrate request counts from `http_requests_total{handler,code,method}` to `dcl_http_requests_total{component,route,status_code,method}`. Select `component="stats"`, `"connector"` or `"camera-reel"`. Use `dcl_http_request_duration_seconds` for latency and `dcl_http_request_declared_size_bytes` for requests carrying a valid Content-Length. Review bucket-based latency alerts, especially Camera Reel's former 60-second bucket. The new histogram's largest finite bound is 30 seconds. Social and Worlds continue to publish their existing HTTP families on the process scrape.

## Recording and querying

Declare a service counter, gauge or histogram in its owning crate and export its descriptor to the binary's registry. Supply fixed label keys and bounded values. For example, a Builder failure can identify a lookup layer. A collection ID belongs in a log. The registry validates conflicting declarations at startup and applies declared histogram buckets before observations begin.

The HTTP layer records completed requests at response headers and labels an interrupted handler `status_code="cancelled"`. The request-size histogram uses a valid Content-Length header. For request rate by component and route, use `sum by (component, route) (rate(dcl_http_requests_total{job="catalyst-serve"}[5m]))`.

Each serve replica samples the same NATS snapshot for `dcl_connector_peers` and `dcl_connector_islands`. For one deployment, use `max(dcl_connector_peers{job="catalyst-serve"})` to report the largest observed replica value. Use the same aggregation for islands. When querying several independent fleets, group by the discovery labels that identify each fleet.

The WebSocket wrapper counts application text and binary payloads on read and successful outbound flush. A successful flush measures progress at the local transport. Application-level delivery acknowledgements and queue drops have their own semantics. Explore Auth exposes Socket.IO namespace sessions and handled inbound events at the boundary provided by its Socket.IO library. Sessions can also use polling.

Periodic workers report completed runs by outcome, duration, active passes and the last successful completion. A separate gauge counts leases held by the process. A process restart resets its local series. A worker that hands off to another process starts producing observations on the new owner's target.

## Locating a metric

| Measurements | Source under the repository root | Canonical target |
| --- | --- | --- |
| HTTP count, duration, declared size and in-flight requests | `library/dcl-metrics/src/http.rs`, ingress middleware installed by `mount_services` in `services/catalyst/comms/src/stats/http.rs`, component labels assigned in `services/catalyst/src/serve_routes.rs` and the service routers | Serve |
| WebSocket transport counts, bytes, errors, active connections and lifetime | `library/dcl-metrics/src/ws.rs`, wrapped around each upgraded socket | Serve |
| Operating-system measurements available on the host | `library/dcl-metrics/src/process.rs`, sampled when rendering the scrape | Each role |
| Builder collection resolution, Shop profiles, Explore accounts and Socket.IO | `services/catalyst/{builder,shop,explore}/src`; the serve registry lists each declaration group | Serve |
| Content synchronization, pruning and upload, storage spool and copy activity | `services/catalyst/{content,storage}/src` | Serve |
| Worlds deployments and uploads, Social RPC and other Social operations | `services/catalyst/worlds/src/core/metrics.rs`, `services/catalyst/social/src` | Serve |
| Gatekeeper, presence, Archipelago connector and its cached diagnostics | `services/catalyst/comms/src/{gatekeeper,presence,archipelago}` | Serve |
| Portal chain reads and supervised jobs | `services/catalyst/chain-index/src/portal`, `services/catalyst/jobs/src/instrumentation.rs` | The serve or index process executing the work |
| Index cursor lag, publication, mirror progress and freshness | `services/catalyst/src/index/metrics.rs` | Index |
| Pulse handshakes, simulation and clustering | `services/catalyst/comms/src/pulse/metrics.rs` | Pulse |
| Message delivery, latency, batch failures and connectivity | `services/catalyst/message-router/src/metrics.rs` | Message router |

The complete declaration groups for serve are composed in `services/catalyst/src/serve_lifecycle.rs`. A family with no observations may be absent until its first use. For the content sync worker, `time() - content_sync_last_heartbeat_timestamp_seconds` gives the age of its last successfully persisted heartbeat in seconds.
