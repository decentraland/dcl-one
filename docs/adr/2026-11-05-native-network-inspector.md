# ADR: Native engine Network inspection

- Status: Proposed
- Date: 2026-11-05

## Context

The desktop Explorer performs content and service HTTP requests in Rust. Its embedded
browser's Network panel cannot observe them. Engine engineers and deployment integrators
need request paths, timing and terminal results when diagnosing native transfers.

## Decision

Expose an optional read-only Chrome DevTools Protocol target from the existing `platform`
crate. The desktop executable starts it only for an explicit `--network-devtools <port>`
argument. Deep links cannot enable it. No new crate or dependency is required, and this
feature has no dependency on deployment gateway configuration.

The target binds to loopback, checks Host and Origin, bounds client count and event queues,
and disconnects slow subscribers. It retains no history for future subscribers and cannot
modify requests. Without the flag there is no listener or event queue.

Preserve normalized request paths so distinct content hashes and resource identifiers stay
visible. Strip userinfo, query strings and fragments; omit headers and bodies. Bound emitted
URLs to 4,096 characters. Path values remain visible to the operator who enables inspection.

Trace each admitted content attempt through body consumption without changing the existing
scheduler permit lifetime. Service JSON wrappers keep the trace through body consumption
as well. Completion reports consumed body bytes; a deliberately discarded response body
reports zero. Redirects expose the initial request and final response only.

## Coverage and next work

The target observes content-budget downloads, active-entity POSTs, font metadata, realm
about/scenes requests, registry profile requests, signed comms login and gatekeeper adapters.
It does not cover every native HTTP caller. Direct reqwest paths including wallet sign-in,
world access and profile uploads remain outside it. Scene and HUD Fetch run in the separate
scene runtime process; joining those events requires an explicit IPC contract. The headless
executable does not start the target.

Cache hits, admission waits, deduplicated readers, initiators, incremental body events and
DNS/TLS timing are not represented. Comprehensive transport coverage remains follow-up work
for this draft. The protocol fixture validates discovery, connection restrictions, request
completion and failure, cancellation, URL display rules and scheduler lifetime. A packaged
native session remains the acceptance check for the Chrome frontend.
