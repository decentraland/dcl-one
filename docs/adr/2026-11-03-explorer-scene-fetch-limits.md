# ADR: Each Explorer scene owns its Fetch limit

- Status: Accepted
- Date: 2026-11-03

## Context

Scene scripts choose the destinations and lifetimes of their Fetch requests.
Several scenes run in one native sidecar or server process. Browser scenes
each run in a dedicated worker. A scene waiting for a slow endpoint must leave
other scenes free to send requests.

## Decision

Each scene may have 32 outstanding Fetch or signedFetch calls in total. An
additional call fails immediately. A client scene sends up to five at once and
queues the rest in arrival order. A hosted scene can send all 32 at once. The
native isolate owns its counters in Deno's `OpState`. Each browser sandbox
owns its queue for the worker's lifetime.

Client requests have a 30-second deadline and hosted requests have 15 seconds.
On native, calls awaiting permission count as outstanding. The deadline starts
on approval and covers admission, address validation, upload, headers and
buffering the response body. Rejection or cancellation closes the upload.

On web the deadline starts at Fetch invocation and ends when headers arrive.
The caller can read or cancel the returned browser stream independently. A
recoverable error event keeps the worker and its requests alive. Teardown
aborts its transfers.
