# ADR: Shared Ethereum RPC readers

- Status: Accepted
- Date: 2026-11-13

## Decision

The existing `catalyst-chain-index` crate owns Ethereum JSON-RPC log and header transport in its `rpc` module. LAND authorization and Shop consume that module directly. The client holds the outbound transport and endpoint; callers supply address and topic filters. It does not hold LAND registry settings.

The shared module owns response parsing, bounded log-range retries, header batches and generic transport, RPC and request-budget errors. LAND authorization retains registry filters, timestamp validity and batch retry policy, request accounting, cursor advancement, fold readiness and repair. Its error conversion retains domain error distinctions and messages. Shop retains receipt checks, index folds, reorganization handling and Credits settlement policy.

This keeps shared chain-reading mechanisms below domain policy without another crate, outbound client, pool or deployment. Existing layer dependencies already permit both consumers to use chain-index. No schema, HTTP route, authorization rule or request budget changes.

The Shop chain walk reads through the `logs` module's `LogSource`. A source answers the chain id it serves, the newest block, the newest finalized header, a header by height, a window of logs for a set of filter groups with the block it proved complete, and the block times of those logs. Each read reports the requests it took, as the walk charges them to its budget: for JSON-RPC, HTTP requests, a batch counting one. That count is not a provider's bill and is not comparable across sources as is. `RpcSource` is the JSON-RPC implementation: one request per chain id, head, finalized or header read, and the window and block-time reads described above, including halving the address chunk when the endpoint refuses the count. The walk keeps the target block, confirmation depth, cursor, request budget, cancellation, reorganization handling and publication, and its backfill lanes read through the same source.

## Validation

The chain reader parser tests run in chain-index. LAND integration tests exercise range hints, budget exhaustion, missing timestamps, batch fallback and retry, replay and publication. Shop tests exercise its filters, reorganization boundaries and Credits index consumers. A Shop integration test pins the order of the JSON-RPC requests the walk sends for one history, with each log request's block range and address count and each batch's headers: budgeted passes, an address-count refusal, missing timestamps, a pass with nothing new, a backfill lane and a reorganization rewind. The module dependency check enforces the existing layer graph.
