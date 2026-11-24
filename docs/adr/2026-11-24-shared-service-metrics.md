# Shared service metrics

- Status: Accepted for implementation
- Date: 2026-11-24

## Context

Catalyst domain crates record into the Rust metrics facade. Serve, Pulse and the message router configure exposition independently, and the standalone index worker enables its listener and lag monitor when INDEX_METRICS_BIND is set. HTTP instrumentation covers selected services. WebSocket implementations measure different stages of delivery.

## Decision

Use a shared Rust library, dcl-metrics, for process recorder setup, HTTP observations, WebSocket observations and service metric declarations. Each executable initializes its recorder before starting workers. Each replica exposes its own private scrape target. Service crates own domain metrics and record into their host process. Keep existing metric names, units, histogram buckets and compatibility views available to their consumers.

Count an HTTP request once, through response headers, including an upgrade handshake. Track established WebSocket connections through closure or cancellation. Count messages at an identified transport boundary and distinguish send failures from successful writes. Use bounded labels and matched route templates. Preserve per-process counters across compatibility-view resets.

## Consequences

Prometheus discovers individual processes. Domain libraries need no additional listeners. Instance identity belongs to target discovery. Metrics for the index worker require a separate listener address. Compare parsed scrape output and exercise success, failure and cancellation paths when changing instrumentation.

## Scrape targets

A scrape reports observations made by one running process. For example, a Builder request and an ordinary Social WebSocket handled by the same serve replica appear together on that replica's port 5006. An index job handled by another process appears on that process's port 5008. Scaling serve to three replicas gives Prometheus three serve targets.

| Role | Address setting | Default target |
| --- | --- | --- |
| `catalyst serve` | `SERVE_METRICS_BIND` | `127.0.0.1:5006/metrics` |
| `catalyst pulse` | `PULSE_METRICS_BIND` | `127.0.0.1:5005/metrics` |
| `catalyst-message-router` | `MESSAGE_ROUTER_METRICS_BIND` | `127.0.0.1:5007/metrics` |
| `catalyst index` | `INDEX_METRICS_BIND` | `127.0.0.1:5008/metrics` |

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

The Social RPC route offers a filtered compatibility view. Stats and connector retain their compatibility routes and reset settings. Camera Reel also retains its compatibility HTTP metrics route. Use canonical targets for fleet totals. During migration, select the scrape job explicitly when comparing those views.

## Recording and querying

Declare a service counter, gauge or histogram in its owning crate and export its descriptor to the binary's registry. Supply fixed label keys and bounded values. For example, a Builder failure can identify a lookup layer. A collection ID belongs in a log. The registry validates conflicting declarations at startup and applies declared histogram buckets before observations begin.

The HTTP layer records completed requests at response headers and labels an interrupted handler `status_code="cancelled"`. The request-size histogram uses a valid Content-Length header. For request rate by component and route, use `sum by (component, route) (rate(dcl_http_requests_total{job="catalyst-serve"}[5m]))`.

The WebSocket wrapper counts application text and binary payloads on read and successful outbound flush. A successful flush measures progress at the local transport. Application-level delivery acknowledgements and queue drops have their own semantics. Explore Auth exposes Socket.IO namespace sessions and handled inbound events at the boundary provided by its Socket.IO library. Sessions can also use polling.

Periodic workers report completed runs by outcome, duration, active passes and the last successful completion. A separate gauge counts leases held by the process. A process restart resets its local series. A worker that hands off to another process starts producing observations on the new owner's target.

## Locating a metric

| Measurements | Source under the repository root | Canonical target |
| --- | --- | --- |
| HTTP count, duration, declared size and in-flight requests | `library/dcl-metrics/src/http.rs`, assembled in `services/catalyst/src/serve_routes.rs` | Serve |
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
