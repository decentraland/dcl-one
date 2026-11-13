# ADR: Pulse owns Archipelago statistics

- Status: Accepted for ownership, process boundaries and metric constraints
- Date: 2026-11-10

## Decision

Pulse owns the in-memory peer population used by the Archipelago statistics APIs.
The connector in the primary `catalyst serve` remains responsible for `/ws`, signed
socket sessions and delivery of Gatekeeper assignments. It forwards statistics
observations over the existing NATS connection. Gatekeeper retains admission,
LiveKit credentials, moderation and takeover cleanup.

The statistics projection is separate from native session ownership and clustering.
A statistics observation cannot claim a session, assign a room or authorize a token.
Native connections alone do not add peers to the legacy statistics population.
The projection preserves heartbeat receipt times, exact peer identifiers, insertion
order and deletion on each authenticated socket disconnect. It has no peer expiry.

Acceptance covers the ownership and process boundaries above. The replication
parameters below describe the current implementation; their deployment capacity
and read-completion requirements still need validation and acceptance.

Runtime measurements describe the Rust implementation and its processes. Node.js
metrics are not fabricated, and embedded V8 use does not imply Node.js runtime
telemetry or synthetic engine measurements.
Application metric names, values and labels, and the public HTTP contracts, remain
compatibility requirements except for connector method-label admission below.
These metric choices do not establish deployment readiness.

Connector HTTP metrics record only a finite set of known HTTP methods, normalized
to lowercase. Requests with unknown methods retain their HTTP behavior but create
no metric observations. This bounds method-label cardinality without introducing
a replacement label. This omission is an accepted application-metrics difference;
it does not change connector method routing or responses.

## Replication behavior and limits

Pulse publishes ordered statistical changes and watermarked complete recovery
snapshots over core NATS. A reader applies each contiguous change before its
subscription callback completes. Delivery between processes still takes time.
Each transport frame is bounded; a receiver exposes a recovery transfer only when
it is complete. A revision gap retains the previous complete view until recovery.
Reader startup and reconnect recover on a subsequent complete publication.
Pulse restart starts with empty state;
subsequent heartbeats rebuild it. Core NATS remains at-most-once and no persistent
queue or database is introduced.

Recovery is requested every five seconds, checked between serialized transfers.
One checkpoint captures a finite revision prefix and its complete state. The
publisher sends that prefix in order, then the checkpoint, then later changes;
new arrivals cannot postpone the checkpoint. Publication time includes the current
transfer, the captured prefix, encoding and the checkpoint transfer. Each send has
a 15-second deadline, but encoding is outside it; five seconds is not an end-to-end
recovery deadline. Backlog overflow or failed publication requires a new complete
recovery and can lose intermediate transitions. Only one checkpoint is retained
by a publisher. Encoding is awaited sequentially; a cancelled publisher can leave
its already-started blocking encoding running while a successor starts.

The current transport supports recovery payloads up to 64 MiB, with at most two
incomplete transfers per reader and a 15-second transfer lifetime. Larger states
are rejected as a whole; the last complete view remains visible. Readers retain
fences for the 4,096 most recently retired Pulse incarnations plus their active
producer; at capacity the oldest fence is forgotten, so a new Pulse incarnation
is never refused. An incarnation whose fence was forgotten can again replace a
quiet producer with a complete snapshot. This reader limit is separate from
connector registration. The selected
producer remains selected while valid publications arrive; replacement requires
15 seconds of silence and a valid complete snapshot. This is not leader election.
Deployment capacity and a single active writer remain release requirements.

Connector observations carry a Pulse incarnation and a Pulse-issued generation.
On its first authenticated local observation, a connector obtains that incarnation
through the correlated statistics control capability, requests a candidate grant,
then confirms its exact random challenge. Pulse retains one active generation and
cursor plus one candidate, rather than a lifetime set of connector UUIDs. A checked
monotonic counter issues generations; exhaustion fails closed. Beginning a candidate
does not replace the active writer, and an overwritten candidate cannot confirm.
Confirmation is idempotent without resetting the cursor. Old generations cannot
publish or confirm after handover; old-incarnation controls and events are rejected
after Pulse restart. This is a writer fence, not leader election or peer expiry.
Pulse records rejected observations in one counter without producer or reason
labels. This exposes malformed, stale or fenced input without per-event warnings,
delivery acknowledgments or automatic writer reclamation.

Snapshot source headers only mark a connector's registration dirty. On its next
authenticated local observation it probes the actual incarnation using a correlated
reply, preserving its cursor if Pulse has not restarted. Passive replicas do not
claim on startup, reconnect or snapshots. An uncertain registration can retry its
exact candidate on the next observation; the observation whose registration failed
is counted as lost and is not replayed. A submitted control can mutate Pulse after
its caller times out; an idempotent retry preserves the cursor and cannot revive a
superseded challenge. Registration and publication share the
existing absolute observation deadline, including the two-second terminal budget.
Control uses the same bounded request queue and readiness/revision fences, and can
fail during an outage; successful registration does not acknowledge later event
delivery. The observation and control subjects require matching connector/Pulse
deployment and permissions; the internal observation codec has no older-version
bypass. No persistent registration or population store is introduced.

Orderly connector shutdown stops admission and drains socket handlers, writers
and observations concurrently before stopping jobs. The observation queue stays
open until handlers finish or its five-second drain budget expires. Each stage
has a deadline; forced loss and unconfirmed flushes are reported.

Disconnect observations wait on the connector's bounded FIFO while teardown
retains its handler admission permit. One two-second deadline covers terminal
queue admission, queued age and bus submission, so statistics cannot hold that
permit indefinitely. Statistical backpressure does not retain dead signaling
authority or delay its release. The bus has a disconnect-only
submission path on the existing subject and outbound queue: it retains at most
one unsubmitted head across link rebuilds without requiring admission readiness.
Cancellation or expiry prevents an unstarted attempt; a started SDK attempt is
never replayed, including after an uncertain result. Its SDK wait is bounded by
the earlier of the terminal deadline and the existing broker deadline. A link or
revision change during submission yields an unconfirmed result. Shared subscription
activation keeps its own broker budget; terminal expiry does not declare the shared
link unhealthy. Expiry is checked again before SDK entry. The five-second observation
drain closes admission before counting forced in-flight and queued loss. SDK
submission does not acknowledge broker delivery or Pulse consumption. Process death and forced shutdown can
still leave statistical peers; producer silence does not expire them.

A read-only statistics view in `serve` supplies HTTP handlers, discovery and the
presence archive. Hot-scene content enrichment uses the existing content database
pool in `serve`. Social's topology view and LiveKit occupancy remain separate from
this legacy API projection.

Content owns the ordered JSON representation used by statistics. Its selected
metadata read preserves the order retained by the existing JSON column, using
the same pointer selection as ordinary content reads and projecting only the
referenced local thumbnail file. Stats
and the archive share that representation; ordinary content APIs keep their
existing projection. Strict decoding preserves literal number-marker objects
and uses a temporary key index to avoid quadratic duplicate-property scans.
Validator hashing explicitly retains its existing parsing and writing behavior.

This read boundary does not recover order lost before persistence or replace
selected metadata with raw entity blobs. It retains the existing finite-number
and Unicode-scalar parser domain. Source corpus, remote cache ordering and
broader JavaScript parsing compatibility remain separate verification work.

Every serve exposes the read-only statistics projections. Complete standalone stats and connector mounts use `/archipelago-ea-stats` and `/archipelago-ea-ws-connector`, matching the gateway's subdomain-prefix routing. Shared root content status remains separate; only the primary serves connector sockets. Shared-root `/stats/*` aliases remain separate from the standalone Stats route set. Existing Stats `/ws` and `/ping` extensions remain explicit compatibility constraints for host routing.

The connector retains enforcement room assignments separately from statistics. Fenced delivery records an issued room under its existing ownership guard after Pulse and durable SQL authorization; statistical observations never grant that authority. HTTP, about/realms and each archive pass retain one coherent snapshot, including across content enrichment I/O.

## Placement

The producer belongs in `comms::pulse`; shared observation and snapshot contracts
belong in `comms::protocol`; the read-only view and HTTP projection belong in
`comms::stats`. These are modules in `catalyst-comms`, not new crates or processes.
The connector's retained room-assignment state serves moderation and socket safety,
not statistics. Pulse imports no Gatekeeper implementation and receives no LiveKit
credential or SQL pool.

## Reasons and constraints

Keeping `/ws` beside Gatekeeper preserves local admission and assignment delivery.
Moving statistical state to Pulse gives that projection one writer while allowing
existing public HTTP paths to stay on the shared API process. Moving credentials
or content SQL into Pulse would expand its responsibilities without helping the
statistics contract.

Compatibility is assessed against pinned contracts for both the statistics and
connector surfaces, including methods, headers, errors and socket lifecycle.
Unknown or divergent behavior must remain visible in review evidence. Source
inspection and this ownership decision do not establish a compatibility or release
verdict. Rollout requires matching producer and reader protocols and NATS subject
permissions; it is separate from implementation approval.
