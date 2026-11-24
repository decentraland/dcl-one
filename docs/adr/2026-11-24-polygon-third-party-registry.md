# ADR: Polygon third-party registry provider reads

- Status: Accepted
- Date: 2026-11-24

## Decision

Shop walks the Polygon Third Party Registry from its first deployment with
the market chain indexes. Each supported event records the provider state
after that event and, for slot consumption, its receipt. Versions commit
with the chain cursor. Rewinding drops the affected versions, restoring the
preceding provider state without a separate replay before recovery can
continue. Approval follows the contract's event flag independently of the
Merkle root. Malformed metadata clears parsed fields and linked contracts
while preserving the raw event metadata.

The Lambda provider list and manager and provider detail queries select the
latest provider versions at the same published block. A walker can commit
newer versions while readers continue using that publication. The cursor
must cover the complete Polygon history through its published block, be
current and be outside divergence and quarantine.

The indexed maximum less consumed slots gives the remaining chain capacity.
The publish service also owns submission reservations incurred before an
on-chain receipt exists. The web publish flow retains that service's
reservation and submission checks. For registered Polygon providers, the
live registry balance caps availability when Catalyst cannot supply indexed
slots. Providers outside the Polygon registry use their configured publish
flow.

## Consequences

An API replica needs access to the shared registry cursor and versions in
PostgreSQL. Initial catch-up, an invalidated publication or a publication
older than 30 minutes makes indexed reads unavailable. When a subgraph is
configured, the existing Lambda provider routes use it while the native
publication is unavailable or the index is disabled. Testnet mirrors
default to the Polygon registry subgraph. Manager and slot details require
a native publication.
