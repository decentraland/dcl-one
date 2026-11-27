# Readiness for a Catalyst API replica

## Decision

Use `GET /health` on each `catalyst serve` replica to decide whether to send it traffic. HTTP 200 and a top-level `status` of `pass` mean all required checks passed. A required check that fails or times out produces HTTP 503 and `fail`. Each entry in `details` reports its own `status` and a boolean `required`. A 200 response can contain failed diagnostics.

Each service owns its readiness predicate. Serve runs the applicable checks concurrently in process, with a two-second deadline per check. Concurrent probes share one evaluation and reuse the result for one second. Responses carry `Cache-Control: no-store`.

| Required check | When the replica can accept traffic |
| --- | --- |
| `server` | Its listener has started and is accepting requests. |
| `social-rpc` | Its initial fanout subscription is ready to admit connections, or fanout is disabled. |
| `map` | Its first grid is loaded. This check applies to replicas configured with the sources for a mainnet map. |

## Shared dependencies

Moving a request to another replica helps when the failure is local. The following signals are visible to operators and excluded from the HTTP routing decision.

| Diagnostic | Meaning of a failed check |
| --- | --- |
| `content` | Sync reports bootstrap or failure. Followers mirror the leader's published state and can all report bootstrap during lease takeover. The same routing policy applies on both sides of the handoff. Nodes explicitly serving unsynced content omit this check. |
| `comms` | The configured Pulse heartbeat appears unhealthy. A Pulse outage or clock skew can affect this observation across replicas. |
| `postgres` | A query through the request pool failed or exceeded the deadline. This can indicate local pool saturation or a shared database outage. Withdrawing a busy replica would move its load onto peers. |

Alert on these diagnostics separately. During a database outage, database-backed requests can fail while the API stays in rotation and other routes continue serving.

## Deployment

Configure load balancers to probe `/health` directly on each backend and withdraw individual replicas on 503. Set the HTTP probe timeout above the two-second check deadline (for example, five seconds) so the API can return a result even when a diagnostic times out. The apex gateway samples one backend per request. If every target fails a required check, the load balancer's all-targets-unhealthy policy determines whether to fail open or return an unavailable response. Choose and verify that policy at deployment. Use a process or listener liveness probe for restart decisions. Nginx has its own `/_gateway/healthz`. Existing service probes retain their published response formats.

The development supervisor starts Pulse once `/status` confirms the API is listening. Offline and testnet stacks then require both HTTP 200 and passing PostgreSQL and comms diagnostics from `/health` before reporting the stack ready for players. The local mirror uses `/about` during startup so it can launch while its historical map indexes catch up. Sites and separately deployed workers have their own lifecycle checks.
