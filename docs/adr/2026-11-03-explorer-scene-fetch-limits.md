# ADR: Each Explorer scene owns its Fetch limit

- Status: Accepted
- Date: 2026-11-03

## Context

Scene scripts choose the destinations and lifetimes of their Fetch requests.
Several scenes run in one native sidecar or server process. Browser scenes
each run in a dedicated worker. A scene waiting for a slow endpoint must leave
other scenes free to send requests.

## Decision

Each client or hosted scene may send five Fetch or signedFetch requests at once.
The two APIs share those slots. Additional eligible calls wait in arrival order.
A scene can have 32 outstanding calls in total, including the five active requests.
The 33rd call fails immediately. The native isolate owns its counters in Deno's
`OpState`, and each browser sandbox owns its queue for the worker's lifetime.

Client requests have a 30-second deadline and hosted requests have 15 seconds.
On native, unanswered permission prompts count toward the 32 outstanding calls
to bound the pending requests and uploads a scene can retain. The deadline
starts on approval and covers admission, address validation, upload, headers
and buffering the response body. Aborting a call releases its outstanding
count, including while awaiting permission, and any slot it holds. Rejection
or cancellation closes the upload.

On web the deadline starts at Fetch invocation. Both the deadline and the slot
end when headers arrive. The caller can then read or cancel the browser stream
independently. An open body can still occupy a browser connection shared with
other requests to that host. The queue schedules the scene's Fetch APIs.
Browser facilities such as module imports use the browser's own scheduling.
A recoverable error event keeps the worker and its requests alive. Teardown aborts
its transfers.
