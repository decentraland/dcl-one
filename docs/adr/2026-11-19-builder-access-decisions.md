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
  collection or item take that value, and the curation status writes take the
  admin proof, so a route that skips the decision doesn't compile. Only the
  public search of published collections, the published collections'
  contract addresses (`GET /v1/addresses`) and the file reads by hash take
  neither, by design. A write decides again inside its own transaction, after
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
  read. A write the committee admitted is the one exception: it reads the
  committee again inside its transaction, holding its locked rows for at
  most the read's 10-second bound.
- **One owner.** A standard collection's owner is its chain creator once the
  copy holds it, and its stored `eth_address` before. The owner is derived on
  each request, so `POST /v1/collections/{id}/owner/sync` stores nothing and
  answers the collection's body. An item in a collection is decided by its
  collection: its owner, an on-chain manager or the provider's manager. The
  item row's stored `eth_address` decides only for an item with no
  collection, so after a transfer the chain creator holds the roles on the
  collection's items and the wallet stored before holds none, and the wallet
  that last saved a third-party item decides nothing.
- **The lifecycle routes.** Only the owner of a standard collection sets its
  lock. The owner, an on-chain manager or the provider's manager deletes a
  collection and accepts its terms of service. Those, or a current committee
  member, publish a standard collection and sync its owner; the committee's
  admission stands when the publish decides again inside its transaction.
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
  more than 1,000 members can't tell: that answer lasts 30 seconds, every
  decision the committee could change answers 503, and so do
  `GET /v1/committee` and the console's committee list, which answer from
  the same read. The node can't check which chain the subgraph indexes; the
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
  third-party collection has no owner. A provider's managers read, save,
  delete and accept the terms of every collection and item of the provider,
  and read the files those items name in a name whose writer holds the
  bytes.
- **Virtual third parties.** A wallet changes a provider's virtual third
  party (`PATCH /v1/thirdParties/{id}`) or removes it (`DELETE`) only when
  the virtual third party lists it and it manages the provider by the rule
  above (`access::changes_virtual_third_party`), so a change never acts for
  a wallet the registry leaves out of an approved or unapproved provider.
  Both read the registry and answer 503 while it can't tell, and both decide
  inside their transaction, with the provider locked.
- **Third-party creates.** A save that creates a third-party collection
  under a provider the registry or a virtual third party has is a
  provider manager's. Under a provider neither has, any signer creates it,
  and the save creates the provider's virtual third party with the signer
  as its only manager, in the same transaction.
- **The registry, read live.** The builder reads the whole third-party
  registry (`thirdParties`, every provider, approved or not, with its
  managers, merkle root, item allowance, approval, kind and metadata)
  under a 10-second bound. On Polygon it reads the shop's published
  registry index at its published block
  ([registry index](2026-11-24-polygon-third-party-registry.md)), and can't
  tell while that index has no current publication. On Amoy, which has no
  index, it reads through the chain index's subgraph client from
  `THIRD_PARTY_REGISTRY_L2_SUBGRAPH_URL`, else the builder's network's
  registry subgraph on Decentraland's subgraph worker. A write's decision, and every
  check a write repeats inside its transaction, accepts only a read that
  started after the decision began, so a manager the registry stops
  listing loses its writes as soon as the index publishes the change, or
  the subgraph indexes it. A
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
- **Curations.** A collection's curations, and an item's, are read and
  written by a wallet with a role on the collection or by a current
  committee member. The collection must exist (404) and be published: a
  standard one on chain, a third-party one with an item curation (409);
  only then are the roles asked, so a stranger gets 401. The committee is
  asked only when no role answers: while the committee can't tell, the
  committee's curation listing and every decision only the committee
  can make answer 503, never a narrower view. Which status change each caller may make is
  one table (`builder/src/curation/transitions.rs`); a write asks it again
  inside its transaction, with the collection row locked. There it also
  reads the committee again: a curator the committee admitted, and the
  assignee a write names, must still be members (401 otherwise). A review comment, appeal or retry asks the committee
  again only when no role on the collection still admits its signer.
- **Admins.** The operator's routes, the item status routes and the
  console's under-review listing (`GET /v1/collections/curation`), admit
  the curation console's bearer (`CATALYRST_BUILDER_ADMIN_TOKEN`) and a
  signed request from a `BUILDER_ADMIN_ADDRESSES` wallet
  (`builder/src/auth.rs` `authorize_admin`). Neither opens another route or
  grants a role on collections, items or their curations. On the item status routes the operator sets the status of
  an item curation of a published collection (404 when the collection is
  absent, 409 when it isn't published) with the moves the committee has on
  an item's status, asked of the same table as the operator, in one
  transaction that locks the collection and decides again, each move
  recorded in `ops.admin_audit`.

## Consequences

- A wallet the registry stops listing as a provider's manager is refused
  on its next request once the registry index has published the change, or
  on Amoy the registry subgraph has indexed it. A
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
  grant answers 503 rather than 401, and `GET /v1/committee` answers 503
  rather than an empty list. Published content stays readable as any
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
- A wallet the committee no longer lists reads and changes no curation,
  even one it is assigned to or decided, and a write the committee admitted
  commits nothing once the subgraph drops the curator or the assignee before
  the write's transaction reads the committee; a committee member who also holds a
  role reads and writes as that role until the table asks for the
  committee.
- A new role or source is one change to `access.rs`, which the authorization
  matrix test (`builder/src/tests/access_matrix.rs`) covers for every routed
  operation.

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
- **A committee that can't be read admitting only the configured admin
  addresses.** It would refuse every member with a 401 as if nobody were on
  the committee while the source is down, and give the admin addresses a
  role on the upstream routes, which they don't have.
- **The committee as every wallet that ever curated.** Past curators would
  keep the role for good.
- **A registry read kept for a time, or one read per provider.** A kept read
  would let a removed manager keep access for its lifetime, and would break
  the old builder web's wait for a new merkle root. Reads keyed by provider
  would let a caller cycling provider ids send one subgraph query per id;
  the whole registry is small enough to read at once.
- **The virtual third party alone before approval.** It would leave the
  provider to whichever wallet claimed its id first, until approval.
