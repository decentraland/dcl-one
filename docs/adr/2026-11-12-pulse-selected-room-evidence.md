# ADR: Pulse selected room evidence

- Status: Accepted for native reservation and reader evidence
- Date: 2026-11-12

## Decision

An active authenticated native Pulse registration reserves its wallet's room
selection, including while its position cannot be read. Legacy signaling
registrations remain independent. Neither a legacy position nor a statistical
observation can override that native reservation.

Pulse shares the IdentityBoard wallet-current map with its assignment readers.
The current peer, session and registration must all match. Native registration
and rebinding invalidate an existing legacy recovery record before publishing
the new current identity. The record retains that invalidation after native
departure. A later eligible completed pass must allocate a fresh assignment
version before legacy authority can return. Pending cleanup, potential exposure
and revocation floors survive the invalidation.

Short critical sections acquire native current state, control state when needed,
then assignment state. Registration uses native current state then assignment
state. No lock crosses broker publication or policy, database or media I/O.
Checked registration allocation precedes replacement, so exhaustion preserves
the prior registration. Assignment version exhaustion withholds new authority.

The canonical snapshot carries selected origin, positive origin-specific identity
and explicit ACTIVE or RETAINED_CLEANUP_ONLY state. Native and legacy owner-v1
identities support positive uint64; legacy intent-v2 candidates use positive
signed-bigint values. These identities never become database ownership epochs.
Rust persists them as exact JSON integers. Generated TypeScript retains its
existing safe-number domain and rejects larger uint64 decoding.

Unknown or old evidence cannot construct an admission stamp. Cleanup-only
evidence can settle exact retained obligations but cannot authorize a new
credential, including after the last cleanup completion. Stamps preserve selected
origin and identity through assignment JSON, signed refresh and subsequent
lookup verification. An unchanged selection can survive pass advancement.

## Legacy readers

The additive internal `archipelago.control.position.v2` subject carries a bounded
closed legacy intent frame. The ordinary connector publishes intent-v2 frames after verified-session
registration; it never claims a realm row merely to register signaling. Both readers use the existing NATS link and one bounded wallet-slot
envelope, with independent ordering and retention for each version. Explicit
withdrawal terminates an epoch; ordinary live expiry permits a still-open producer
to resume the same session and epoch with a greater sequence. Duplicate samples
do not renew freshness.

Mixed v1/v2 input does not establish a common allocator domain merely by sharing
audience text. Without a grounded production domain proof, mixed legacy authority
is withheld. Native reservation still takes precedence. Finite retention is not
permanent replay protection.

A positionless head still retains ordering and session evidence. It therefore
continues to withhold mixed-version authority until its retention ends; omitting
it from the conflict check would bypass the required common-domain proof. The
current live horizon is 10 seconds, followed by 300 seconds of tombstone
retention after expiry processing. This reader supports no seamless v1-to-v2
producer cutover during that interval. The ordinary producer emits v2, and native reservation is unaffected by
conflicting legacy heads. Residual v1 input must expire or be reconciled before
legacy v2 service is enabled.

## Consequences and remaining work

The connector commits each verified signaling session before room work. Same-session
replacement follows successful authentication completion; distinct sessions remain
connected. A fresh valid heartbeat makes a transport eligible for the wallet's
completion-ranked v2 candidate. Candidate promotion allocates a fresh epoch; candidate
evidence cannot authorize credentials.

Gatekeeper's selected realm coordinator owns acquisition, renewal and conditional
settlement on the shared SQL pool. It preserves wallet-before-realm locking and
compares the complete known predecessor. Native selection works without a native
signaling socket. Fresh unknown foreign owners remain conflicts; a fresh external
same-session assignment can be borrowed read-only after its current stamp validates.
Only a known exact owned predecessor or lapsed row can be replaced. Transport
closure detaches its local delivery binding; it cannot release another owner.
Absent positive selection, renewal stops and the existing 90-second lease lapses.

Dispatched claim, renewal and assignment publication retain immutable before/after
facts across timeout and cancellation. Reconciliation reads under the same wallet
lock, conditionally recognizes the attempted result or unchanged predecessor and
never restores a predecessor. Uncertainty retains its existing subscriber work
credit; settlement can reuse that credit at capacity. Owned facts use the existing
peer-state bound, while one retained sample per live transport, the 1,024 outbox
and 64-frame writer bounds remain. The feed job owns at most 32 candidate allocation
futures, one per wallet, with no per-heartbeat spawned task.

Selected retry obligations survive read-only borrowing, fresh foreign conflicts
and unknown lookups. The internal `peer.{wallet}.selection_retirement` exchange
returns negative lifecycle evidence only. Pulse holds native, control and complete
assignment-view locks together: a fresh same-incarnation completed view must contain
no current selection, blocked record, pending cleanup, native reservation or new
control candidate for that wallet. A hidden or stale current snapshot is unknown.
Every current and unfinished record survives reclamation, so the complete view can
still prove no selected owner after departed-record and control-tombstone cleanup.
Neither their retention age nor SQL lapse is the proof. Requests identify the exact
source version/session and, when previously observed, selected origin and identity;
hint-only requests never fabricate selected identity. Both directions use the existing
64 KiB limit and request deadline. Existing cluster lookups and public endpoints keep
their contracts.

Gatekeeper captures the exact local slot and desired generation before the lookup.
It serializes wallet-before-realm SQL settlement with selected mutations, retains
an uncertain attempt and its credit if settlement fails, and removes the slot only
after rechecking the desired generation, selected identity, exact map entry and
absence of outstanding leases or operations. A delayed reply cannot delete a newer
hint or owner. Retirement changes no realm row and issues no credential: settled owned
facts cease renewal while the finite lease remains; foreign rows are untouched.
The removed slot's exact owned fact stays inside the selected bound, so the wallet's
next selection replaces that known predecessor without waiting for the lease; a new
slot at the bound drops one such fact.
This reclaims the bounded selected capacity across departures under one continuing
Pulse incarnation. A restarted source cannot prove an old source's departure;
cross-incarnation retirement and reconstruction of lost unchanged native hints
remain unresolved. A Pulse-only restart can leave old-source slots
retained until current-source evidence supersedes them; enough such residual wallets
exhaust the selected bound and new wallets are refused island credentials. This is
an unresolved rollout blocker. Slot occupancy and cap refusals are exposed as
`comms_cluster_selected_slots` and `comms_cluster_selected_slot_refused_total`.
Maintenance retries rotate with bounded item, lifetime-credit and running reservations;
event and durable takeover lanes retain headroom. Cold hint-only work is limited to
112 of 128 maintenance items and lifetime credits and eight of 16 maintenance runners;
known-owned and unfinished targets retain the remaining reserve, run first and rotate
separately without overtaking earlier work in the same wallet. Healthy exact owned targets wait
until their 30-second renewal is due, while unknown or unfinished targets retry after
five seconds. Capacity notifications refill batches without a global five-second
throughput limit. The full shutdown settlement scan remains independent of due filtering.
These bounds do not establish cross-incarnation retirement or measured useful throughput.

These paths cannot fence an unrestricted older SQL writer. Deployment still needs
reader readiness, old writer and backend quiescence, actual allocator-domain proof
and exact residual state reconciliation. Physical output admission and runtime
release evidence remain separate requirements. Source implementation alone is not
a rollout approval.

The stored assignment document is also a deployment boundary. Binaries that
predate selected origin and identity reject those fields in their closed JSON
schema. Mixed-version service and rollback to those readers are unsupported;
relaxing a new reader cannot change an already deployed reader. A rollout must
first establish compatible readers and quiesce incompatible writers/backends,
then reconcile retained assignments and cleanup obligations. Deleting assignment
rows is not a rollback procedure and can discard fencing or cleanup evidence.

This is a coordinated maintenance upgrade, not a rolling deployment. The operator
must fence admission and stop incompatible Pulse and gatekeeper processes,
including refresh-serving API replicas, before changing authority versions.
Publication of an image does not enforce that fence. The container
[runtime guidance](../../infra/containers/README.md#pulse-assignment-upgrade)
sets the required cutover conditions; their execution and deployment evidence
remain operator responsibilities.

An old SQL stamp with no selected provenance is rejected by the current refresh
reader with 403. A fresh authenticated admission must establish a new stamp from
current Pulse evidence; neither repeated refresh nor matching stored strings can
upgrade it. Use a rollback image that supports selected identities or a forward fix
after new stamps are written. Returning an incompatible binary to service is
unsupported.

Server-side credential authorization and signed refresh are separate from external
native client delivery. The readers do not establish that an external native
client consumes a Pulse island credential without a signaling socket.
