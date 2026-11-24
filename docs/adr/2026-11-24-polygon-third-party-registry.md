# ADR: Polygon third-party registry provider reads

- Status: Accepted
- Date: 2026-11-24

## Decision

Shop walks the Polygon Third Party Registry from its first deployment with
the market chain indexes. Each supported event records the provider state
after that event and, for slot consumption, its receipt. Versions commit
with the chain cursor. Rewinding drops the affected versions, restoring the
preceding provider state without a separate replay before recovery can
continue.

The Lambda provider list and manager and provider detail queries select the
latest provider versions at the same published block. A walker can commit
newer versions while readers continue using that publication. The cursor
must cover the complete Polygon history through its published block, be
current and be outside divergence and quarantine.

The indexed maximum less consumed slots gives the remaining chain capacity.
The publish service also owns submission reservations incurred before an
on-chain receipt exists. The web publish flow retains that service's
reservation and submission checks. Providers outside the Polygon registry
continue using their configured publish flow.

## Consequences

An API replica needs access to the shared registry cursor and versions in
PostgreSQL. Initial catch-up, an invalidated publication or a publication
older than 30 minutes makes indexed reads unavailable. A disabled index
allows explicitly configured nodes to use the legacy provider list during
rollout.
