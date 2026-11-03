# IPFS admission for each content request attempt

- Status: Accepted
- Date: 2026-11-03

The IPFS loader queues each network attempt through the download priority gate. A shared
download has one ticket for its live readers. When the gate selects that ticket, the transport
takes ownership of the grant through the response body, error or cancellation. The configured
IPFS slot count therefore bounds active transfers even when a download has several readers.

A retry releases its grant before waiting for `Retry-After`. On wake it queues a new ticket
with the hashes of its current readers. The gate uses the priorities in effect when it grants
the next slot. Within the same priority tier the retry goes to the back of the queue, including
when its delay is zero. Waiting downloads can proceed during backoff. Dropping the final reader
withdraws queued work.

Editor imports use the edited scene as their owner. Imposter archives queue without a scene
owner. The priority ordering and treatment of departed owners are described in the
[download priority decision](2026-11-02-explorer-nearest-first-priority.md).
