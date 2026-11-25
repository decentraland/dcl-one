# Readiness for a Catalyst API replica

## Decision

Use `GET /health` on each `catalyst serve` replica to decide whether to send it traffic. The response contains a top-level `status` and a `details` object keyed by check name. HTTP 200 means every registered check passed. HTTP 503 means at least one failed. The failed entries identify which service or dependency needs attention.

Each service owns the predicate for its readiness. Serve registers the checks that apply to its role and configuration, evaluates them in process, and combines their results. Services that use the shared PostgreSQL pool share one database check. A service whose only requirement is a listening HTTP server shares the server check.

The server checks listener startup and drain state. PostgreSQL must answer a query through the request pool. Social RPC reports whether its initial fanout subscription is ready to admit connections. A syncing node or follower uses the content readiness reported by `/about`. A node explicitly serving unsynced content omits that check. A configured comms integration uses the local heartbeat observation. A node configured to build a mainnet map waits for its first loaded grid.

Checks run concurrently with a two-second deadline. Concurrent requests share the result for one second, including failures. The HTTP response carries `Cache-Control: no-store`.

## Deployment

Point load balancer readiness probes at `/health` on each backend. Through the apex gateway the same path samples the replica that handled the request. Probe `/_gateway/healthz` for nginx liveness. Per-service ping, status and health routes retain their published responses for existing clients.

The development supervisor uses `/status` to start Pulse once the API is listening. After Pulse starts, offline and testnet stacks check `/health`, which can then observe its heartbeat. The local mirror profile checks realm availability through `/about` during startup so a fresh installation can launch while its historical map indexes catch up. Its `/health` reports full readiness once those inputs are available. Sites and separately deployed worker processes have their own lifecycle checks.
