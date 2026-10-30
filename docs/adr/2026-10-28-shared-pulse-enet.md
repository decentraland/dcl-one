# ADR: Share the pinned ENet dependency between Pulse and the native Explorer

- Status: Accepted
- Date: 2026-10-28
- Related architecture decisions: decision-54, decision-56

## Context

The Catalyst Pulse server and the native Bevy Explorer consume the same pinned rusty_enet implementation. Both use the three-channel contract, and channel 1 requires sender throttle bypass independent of sequencing and reliable fragmentation fallback. Both senders select the reviewed dependency patch's explicit bypass for that lane while preserving ordinary send boundaries.

Two concrete consumers justify one shared third-party source tree. The client must not depend on services, and the two applications retain separate Cargo workspaces and transport drivers. The repository must build without a sibling checkout or a modified global cache.

## Decision

Keep the upstream rusty_enet crate under `library/vendor/rusty_enet/`. Retain its original package identity and coherent standalone workspace rather than introducing a dcl wrapper or forwarding crate. Both the Catalyst and Explorer workspace manifests depend directly on this same source tree, with their own lockfiles and intended feature selections.

Import exact upstream revision `6fa651d50554f6db8f8a4bd98aaf95ef3238b408`. Preserve the complete MIT license, upstream metadata and build/test inputs. Record source URL, revision, imported and patched checksums, portable patch checksums and update instructions beside the source. The reviewed scoped patch has SHA256 `510728E81855FB5B7EA6AB4CA7E98591E7A77B9E2A85F9B59D4D5C53DCD11655`; further changes require updated provenance and review.

The server and client keep their socket drivers, scheduling, connection lifecycle and ownership. The native client channel-1 callsite uses the independent bypass while preserving sequenced unreliable semantics. Reliable channel 0 and unsequenced channel 2 retain their meanings. Browser WebTransport is unchanged. Sharing the core adds no production process, route, bus message or service dependency.

Add a row to `library/README.md` naming Catalyst and the native Bevy Explorer as consumers. Keep the exact vendor path in ownership.json with its current unassigned ownership. Assigning an owner and regenerating CODEOWNERS requires the separate user decision described below. Regenerate the Explorer forks inventory and Cargo patch inventory from the updated lockfiles using the repository tooling; adjacent vendor provenance remains authoritative for this source import.

## Verification

Review both manifests and lockfiles for resolution to the one tracked vendor tree, intended features and absence of duplicate git sources for these consumers. Update only affected lock entries; do not broadly update dependencies. Verify the standalone vendor workspace has no missing members or hidden build inputs.

CI must run deterministic shared dependency regressions for bypass delivery, ordinary controls, repeated sends of one packet, cross-channel and unsequenced packets, fragment grouping and packet-kind inspection. A narrow native-client mapping regression verifies the bypass is selected for channel 1 while channels 0 and 2 retain their kinds. Provenance checks verify the upstream pin, imported/patched checksums, patch history and full MIT notice.

Native interoperability uses separately pinned harness inputs and an explicit invocation. Missing prerequisites do not count as a native pass. Full Explorer builds, admitted-client journeys and deployment readiness retain separate evidence gates; manifest and callsite review does not establish those results.

## Consequences

One maintained upstream import serves two independent applications without a service-to-client dependency. Updates require review of the shared core and both callsites. A future maintained release with equivalent behavior may replace the import after provenance, feature and wire-behavior review.

The shared host exposes an optional incoming receive policy. Pulse selects a maximum
8192-byte payload, 65536 charged queued bytes and 256 incoming commands per native
peer. Admission reserves payload, fragment bitmap, incoming-command descriptor and
packet descriptor before allocation; queued ownership releases the exact charge on
delivery or removal. Allocator overhead, peer/channel structures, socket buffers and
outgoing queues are outside this charged ceiling. The native Explorer keeps the
default policy disabled, including the existing 32 MiB incoming and outgoing limits.

A structurally valid native payload declaration above 8192 bytes produces a local
terminal event before reassembly allocation. Pulse immediately disconnects with
reason 16 and releases its application state. The parser suppresses pending and
later input for that peer until reset; aggregate byte or command exhaustion only
refuses the command and is not a corruption charge or a new wire reason. The
4097-8192 application corruption band remains unchanged. Native allocation and
mixed-transport capacity under load remain unmeasured.


Channel 1 bypass applies to the full player-state fan-out. Slow observers no
longer receive ENet's probabilistic sender-throttle shedding on that lane;
sequenced-unreliable delivery can still lose packets and does not gain reliable
retransmission. This can increase egress under congestion. Its throughput and
bandwidth impact have not been measured; ordinary throttle controls remain on
other sends.

The imported vendor `.github/workflows/ci.yml` is provenance input, not an active
root GitHub Actions workflow. Root Development Rust jobs now independently verify the exact public pin and
complete pristine file set with `tooling/enet-provenance.py`, without checking
out or executing fetched code. The wrapper retains local patched-byte/reverse
replay checks and the unfiltered vendor runtime, preceded by locked
all-features/all-targets compilation. The imported workflow does not establish
additional CI coverage. Local public 41-file comparison and integrated vendor
compilation are evidenced at the scopes recorded in the Notion compatibility record;
required final-head/default-branch enforcement and full native-client CI remain
separate gates.

Preserve the unassigned ownership entry for `library/vendor/rusty_enet/**`. Concrete maintenance ownership and required-review governance need a separate user decision; this ADR authorizes no CODEOWNERS or branch-protection change. Record that decision in Notion.
