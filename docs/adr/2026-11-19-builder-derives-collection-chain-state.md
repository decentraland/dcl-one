# ADR: The builder derives a collection's chain state

- Status: Accepted
- Date: 2026-11-19

## Context

A standard collection that the builder holds as a draft becomes a contract on
Polygon. From then on the chain decides several things about it:

- whether it is published;
- whether the committee approved it;
- its creator and owner;
- its managers and minters;
- when it was reviewed.

A stored copy of any of these goes stale the moment the chain changes it.
Examples: a deployment the node didn't make, an approval, a manager added or
removed. A collection can also be deployed outside the node's own
publication flow, so the node can't keep a stored flag right by itself.

The node already holds a copy of the Polygon collections subgraph. The shop's
`collections-mirror` job walks the whole subgraph every period and writes
`shop.collections`, and it records each complete pass in `shop.mirror_state`.

## Decision

- **No stored flag.** The builder stores no published or approved flag for a
  collection. It derives those, together with the creator, owner, managers,
  minters and review date, on read. It stores only what the chain can't
  supply:
  - the draft;
  - the contract address computed before deployment;
  - each item's on-chain id;
  - its own publication records.
- **Shared SQL, read only.** The builder reads `shop.collections`,
  `shop.items` (a minted item's chain values) and `shop.mirror_state` by
  shared SQL, Polygon rows only
  (`network = 'matic'`), and never writes them. No crate dependency is added;
  the columns it reads are listed in the root `AGENTS.md` "Ownership" section
  and in `DATABASE.md`, "Builder chain state workload".
- **Published** is any of:
  - the copy holds a row for the collection's contract address;
  - the node holds its own verified publication receipt;
  - an item of the collection has an on-chain id;
  - an item of the collection has an item curation, a third-party item
    submitted for review.

  The node's own records make a collection published as soon as the node
  publishes it, without waiting for the copy.
- **Approved** is the copy's approval of the contract. A third-party
  collection has no contract and is never approved.
- **Fail closed.** Every read starts with the copy's readiness:
  - With no complete pass, every route that needs published or approved
    state answers 503.
  - When the last complete pass is more than 30 minutes old:
    - listings and response bodies still read the copy;
    - every decision that grants access or allows a write answers 503.
  - A decision also refuses data older than 30 minutes:
    - a row, by when the copy last wrote it;
    - an absence, by the start of the pass that saw it.
  - After a complete pass, a contract the copy doesn't hold is not on
    chain.
  - The facts and the write guard can't be reached without a decision view,
    which only the readiness check builds.
- **Bodies and listings merge the copy.** A collection's body, by id and in
  every listing, is its stored row with the copy's name, creator, managers,
  minters, approval, review date and creation and update dates in place of
  the row's once the copy holds its contract. An address's own listing is
  the collections it stored, plus those whose contract the copy lists with
  the address as creator, manager or minter, less those whose contract the
  copy lists without the address in any of those roles. A collection on
  chain that the builder doesn't hold isn't listed.
- **The node computes the contract address.** A standard draft's salt is the
  keccak-256 of its id, and its contract address is the collection factory's
  CREATE2 address for that salt, the forwarder and the collection's
  `initialize` calldata, which the client sends with each save and which must
  name the signer as the collection's creator. A request never sets the
  address. A draft can only map to a contract whose deployment names its own
  signer as creator, so it can't inherit another wallet's collection.
- **A live check where the copy may lag.** An address-changing save needs
  to know of a deployment the copy hasn't seen yet. It asks the Polygon RPC
  (`BUILDER_POLYGON_RPC_URL`) at the stored
  address: `eth_getCode` and the collection's `itemsCount()`, both at the
  finalized block. The address commits to the factory, the salt and the
  `initialize` calldata, so code there is this collection with its creator
  and items. One read per contract runs at a time and its answer is kept five
  seconds; a read never holds a database connection. When the RPC can't
  tell, or the node has none, the save answers 503.
- **The owner's lock.** A collection's stored lock, which the old builder
  web sets right after it deploys, refuses for a day saves of the
  collection, item saves and creations in it and item deletes (423).
- **Deployment.** The node that serves the builder runs the Polygon
  collections mirror (`MARKETPLACE_MIRRORS_ENABLED=true`,
  `MATIC_COLLECTIONS_SUBGRAPH_URL`), or shares a database with a node that
  does.

## Consequences

- **Lag.** A collection deployed outside the node's own flow reads as a
  draft until the copy's next pass. The bound is three times
  `MARKETPLACE_MIRROR_INTERVAL_SECS` (15 minutes at the default 300) plus the
  subgraph's own lag. An edit that changes its name or calldata would give
  the row another computed address, so the row would no longer map to the
  deployed contract: the live check refuses it, and so does the lock while it
  holds. A node without `BUILDER_POLYGON_RPC_URL` refuses every such change
  (503). An approval or a role change shows after the same delay.
- **Saves need a fresh copy; an owner's own read doesn't.** Every standard
  draft holds a computed contract address, and anyone holding its salt and
  calldata can deploy it outside the node. So every save, of a standard or a
  third-party collection, new or existing, decides from a copy at most 30
  minutes old and answers 503 otherwise. A read of a draft is decided the
  same way, except that a manager of a third-party collection's provider,
  or the stored owner of a standard draft whose contract the copy doesn't
  hold, reads it from a copy of any age: no chain fact can change what that
  answer reveals, and the reader's own listing answers the same row from
  the same copy.
- **Nodes without the mirror.** Nodes under the `island` and `journey`
  postures, and the `offline` development profile, run no mirror, so the
  builder's draft routes answer 503 there. The builder logs which condition
  holds each time it changes.
- **Clocks.** The mirror stamps its pass and row times with its host's
  clock, and the builder measures their age with its own. The hosts that run
  them keep synchronized clocks: any skew between them weakens the 30-minute
  bound by its size, and a mirror clock that runs ahead lets a decision accept
  older data.
- **Reorged deployments stay published.** The copy never deletes a row. A
  collection whose deployment a reorganization removed stays published:
  closed for writes, open for anything that reads published content.
- **Older databases.** A database bootstrapped with the stored flags keeps
  the columns until the release script
  `schemas/releases/builder-collection-flags-retired.sql` drops them.
  `DATABASE.md`, "Retired builder collection flags", gives the procedure,
  including closing creator writes while old and new replicas run together.
- **Listing cost.** The committee's whole-table listings derive state for
  every row they read. `DATABASE.md` records their measured cost.

## Alternatives considered

- **A live subgraph read per request.** Each read and decision would depend
  on an external service's availability and latency, with no local
  readiness signal to fail closed on.
- **An RPC `isContract` check per decision.** It tells only whether a
  contract exists, not approval or roles. It also adds an external call to
  every write.
- **Stored flags kept in sync.** The chain changes them without the node, so
  a sync job would be a second copy of the mirror with its own lag. A stale
  flag would also read as a fact rather than as data of a known age.
