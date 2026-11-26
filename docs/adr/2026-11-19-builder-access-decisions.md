# ADR: The builder decides access in one module and reads the current committee

- Status: Accepted
- Date: 2026-11-19

## Context

The builder serves the clients of `builder-api.decentraland.org` with the
roles `github.com/decentraland/builder-server` at `98897af` defines:

- the owner of a collection;
- an on-chain manager of a collection;
- a member of the curation committee;
- a manager of a third-party collection's provider.

Each role comes from a different source: the builder's own rows, the node's
copy of the Polygon collections subgraph, the committee accounts that
subgraph marks, and the third-party registry. A route that checks its roles by
itself can forget one, read one from the request, or grant access when a
source is down.

## Decision

- **One module decides.** `services/catalyst/builder/src/access.rs` decides
  every read and write of a collection or item. It reads only the verified
  signer, the resource ids the request acts on, the builder's rows, the
  chain copy through a decision view, the committee and the third-party
  registry.
- **Grants are values.** A decision returns a value with private fields that
  only `access.rs` builds. The store functions that read or change a
  collection or item take that value, so a route that skips the decision
  doesn't compile. Only the public search of published collections takes
  none, by design. A write decides again inside its own transaction, after
  it locks its rows.
- **Deny by default, fail closed.** A decision grants as soon as one role
  answers yes. It refuses only when every source it needs answered no. When
  a source it needs can't tell, it answers 503 and grants nothing. A source
  is consulted only when the answer depends on it. For published content
  the committee only widens a caller's view: while the committee can't tell,
  a committee member reads published content as any signer does.
- **No connection across another read.** A decision reads the builder's
  rows and the chain copy, returns its database connection, and only then
  reads the committee. It reads the registry before it takes a connection,
  and a write decides again inside its transaction from the same registry
  read.
- **One owner.** A standard collection's owner is its chain creator once the
  copy holds it, and its stored `eth_address` before. The owner is derived on
  each request and stored nowhere. An item in a collection is decided by its
  collection: its owner, an on-chain manager or the provider's manager. The
  item row's stored `eth_address` decides only for an item with no
  collection, so after a transfer the chain creator holds the roles on the
  collection's items and the wallet stored before holds none, and the wallet
  that last saved a third-party item decides nothing.
- **Managers come from the chain.** Only a collection on chain has managers,
  read from the copy. A request body never sets them.
- **Current committee, from the collections subgraph.** The builder reads
  the current committee as `github.com/decentraland/builder-server` reads
  it: the accounts the Polygon collections subgraph marks
  `isCommitteeMember` (`MATIC_COLLECTIONS_SUBGRAPH_URL`, else the builder's
  network's subgraph on Decentraland's subgraph worker), in pages of
  1,000 while a page is full (`builder/src/committee.rs`), through the
  chain index's subgraph client under a 10-second bound. It reads them as
  it reads the registry: a write's decision, and every committee check a
  write repeats inside its transaction, accepts only a read that started
  after the decision began; a read-only decision also accepts a read that
  started up to 5 seconds before it; one read runs at a time and every
  decision waiting when it started shares it. A setting set empty, a
  read that fails, or an answer with an account that is not an address or
  more than 1,000 members can't tell: that answer lasts 30 seconds, and every
  decision the committee could change answers 503. The node can't check which chain the subgraph indexes; the
  deployment points it at the builder's chain.
- **Third-party managers.** A third-party collection's provider is the
  provider id of its URN. Its managers, in the order
  `github.com/decentraland/builder-server` reads its sources:
  - once the registry approves the provider, the managers the registry
    lists;
  - before approval, the managers the registry lists that the provider's
    virtual third party also lists, or every manager the registry lists
    when the provider has no virtual third party;
  - with no registry record, the virtual third party's managers;
  - nobody otherwise.

  The wallet that created a third-party collection decides nothing; a
  third-party collection has no owner. A provider's managers read and save
  every collection and item of the provider and delete its items.
- **Third-party creates.** A save that creates a third-party collection
  under a provider the registry or a virtual third party has is a
  provider manager's. Under a provider neither has, any signer creates it,
  and the save creates the provider's virtual third party with the signer
  as its only manager, in the same transaction.
- **The registry, read live.** The builder reads the whole third-party
  registry (`thirdParties`, every provider, approved or not, with its
  managers, merkle root, item allowance, approval, kind and metadata)
  through the chain index's subgraph client, under a 10-second bound, from
  `THIRD_PARTY_REGISTRY_L2_SUBGRAPH_URL`, else the builder's network's
  registry subgraph on Decentraland's subgraph worker. A write's decision, and every
  check a write repeats inside its transaction, accepts only a read that
  started after the decision began, so a manager the registry stops
  listing loses its writes as soon as the subgraph indexes the change. A
  read-only decision (the grants behind `GET` routes, the public search's
  provider flags included) also accepts a read that started up to 5
  seconds before it, so a page's concurrent reads share one registry read
  and a removed manager keeps reading for at most that long. One read runs
  at a time and every decision waiting when it started shares it. A record the node can't read
  (a field outside the shapes the registry answers, such as a manager that
  is not an address) stays in the registry as a provider nobody manages:
  no managers, not approved, no item allowance, logged once with its id.
  It never counts as missing, so no wallet can claim its provider through a
  virtual third party. A record without an id is left out. A read that
  fails, an answer that breaks the walk (a provider twice, out of order or
  past the page bound), or a setting set empty can't tell: that
  answer lasts 30 seconds, and every decision, listing and view that
  involves a third-party collection answers 503 until a read succeeds.
- **Access checks never write.** A decision reads its sources and changes no
  row, also when it refuses.
## Consequences

- A wallet the registry stops listing as a provider's manager is refused
  on its next request once the registry subgraph has indexed the change. A
  request that involves a third-party collection waits for a registry read
  that starts after it arrives.
- While the registry can't tell, every request that involves a third-party
  collection answers 503 and writes nothing, an address's own listings
  included; requests about standard collections only are unaffected.
- A wallet that a virtual third party lists and that the registry doesn't
  list for the same unapproved provider manages nothing of it, so a wallet
  that claims a provider id before another wallet registers it on chain
  keeps no access once the registry has the record.
- A wallet removed from the committee, or added to it, counts as such for
  writes once the collections subgraph has indexed the change, and for
  reads at most 5 seconds later. A committee decision waits for a subgraph
  read that starts after it arrives, or, for a read, up to 5 seconds before.
- While the committee can't tell, a request that only the committee could
  grant answers 503 rather than 401. Published content stays readable as any
  signer reads it, a committee member included.
- Every by-id read is a decision, so it answers 503 when the chain copy is
  older than its decision bound, as writes do, with one exception: a
  reader whose answer no chain fact can change beyond what it already holds
  (a manager of a third-party collection's provider, or the stored owner of
  a standard draft whose contract the copy doesn't hold) reads it in full
  from a copy that has a complete pass of any age, as its own listing
  already answers it.
- Between a creator transfer and the copy's next pass, the previous creator
  keeps the owner's access.
- A new role or source is one change to `access.rs`.

## Alternatives considered

- **A check in each route.** Nothing stops a route from skipping a role or
  admitting a caller by a value the request supplied.
- **The committee from the collections subgraph, kept an hour.** A removed
  member would keep the role for up to the hour.
- **The committee from the node's own index of the Committee contracts.**
  A node with a new database would walk the contracts' whole history over a
  public RPC, close to an hour on Amoy and longer on Polygon mainnet,
  before it could answer any committee decision; the subgraph answers in
  one request.
- **The committee as every wallet that ever curated.** Past curators would
  keep the role for good.
- **A registry read kept for a time, or one read per provider.** A kept read
  would let a removed manager keep access for its lifetime, and would break
  the old builder web's wait for a new merkle root. Reads keyed by provider
  would let a caller cycling provider ids send one subgraph query per id;
  the whole registry is small enough to read at once.
- **The virtual third party alone before approval.** It would leave the
  provider to whichever wallet claimed its id first, until approval.
