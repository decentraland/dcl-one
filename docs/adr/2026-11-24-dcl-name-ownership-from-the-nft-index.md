# DCL NAME ownership from the nft index

- Status: Accepted
- Date: 2026-11-24

## Decision

Every question about DCL NAMEs (which NAMEs an address holds, who holds a
NAME) goes through one source, the `NameAuthority` of
`services/catalyst/chain-index/src/names.rs`, selected once per node at
startup by the shop (`select_name_authority`) and injected into worlds, comms,
communities and the shop's own routes:

1. a node that walks the nft index (mainnet with
   `MARKETPLACE_NFT_INDEX_ENABLED`, the rule its item routes follow) reads the
   index: the DCLRegistrar rows of `shop.nft`, `ens_subdomain` from
   NameRegistered and `owner` from the transfers
   (`services/catalyst/shop/src/market/names.rs`). It stays the source while
   the index is not current; its reads then refuse;
2. otherwise the marketplace subgraph, when `MARKETPLACE_SUBGRAPH_URL` is set
   (a testnet node, which the walk does not cover);
3. otherwise, on mainnet, the index all the same: a process reading a
   database another one publishes. This case serves lookups (the name
   owner route, the Admin Toolkit's owner lookup, the user NAME lists and
   comms' display names) and authorizes nothing: worlds and communities
   authorize with cases 1 and 2, the sources their startup posture accepts;
4. otherwise none, and every NAME question is unanswered.

Every read names its purpose and checks the Ethereum nft cursor in the same
snapshot as its rows: one statement for a NAME read, so it holds on any
connection (the session a partial upload already holds included), and one
`REPEATABLE READ, READ ONLY` transaction for a profile or an outfit, which
reads its items too:

- An authorization (a world's owner for writes, admission, scene-admin
  rights and read-through signers; comms' Admin Toolkit target; a
  community's creation, federated creation and ownership transfer) needs
  the whole history walked and every walked block folded and published
  (`folded_block = last_block`), within 30 minutes, bound to chain 1 and out
  of quarantine. While a pass has walked a transfer it has not folded, the
  fold still names the former holder, so no holder is authorized.
- A display read (lists, `namesForExtraSlots`, comms' scene names) needs
  the fold published within 30 minutes. A profile's `hasClaimedName` takes
  the profile's own rule instead: the index complete on the networks the
  profile reads, Ethereum always, whatever the age of its publication, as
  its items do (an RPC outage that stops publishing does not refuse
  profiles).

Every answer carries the publication it was read under (folded block and
time, at the database's microsecond precision). A memo files the answer
under it and, on every hit, asks the index whether that publication still
stands for the purpose; after an advance or a rewind the answer is read
again. Worlds' admission memo works this way: it validates the exact entry it
returns, joined in flight included, within the same 3-second deadline as the
read, and an owner the worlds code proves by other means is never noted into
it for an indexed NAME. Comms' display memo keeps no answer of the index and
no failure. A profile reads its items and its NAMEs in one snapshot and is
filed under every network's publication, Ethereum included even when it
names no Ethereum item; an outfit too, and it is not served past the
freshness of what it read.

A label is parsed once, where a raw name enters (the lambdas name owner
route, the Admin Toolkit lookup, a world's name): one `.dcl.eth` stripped,
and for a world only a single lowercase alphanumeric label; the sources take
it bare and strip nothing. Labels are compared lowercased. Two holders of one
label establish nobody: the owner read refuses and a batch leaves the label
out, so a reconcile never takes it for a confirmed absence and never revokes
on it, and neither does it on a source that cannot answer. A wallet's list keeps what the subgraph gave: the minted spelling,
ordered by NFT id (`<contract>-<token id>` as text) and the price of an open,
unexpired order its holder placed.

A source that cannot answer is never "holds nothing": the lambdas answer the
`nft-index` unavailable envelope (or lamb2's 502 for a failed read), the
marketplace API its 503, worlds and communities 503, and comms' scene names
for display only are left empty. `/lambdas/users/{address}/names` alone keeps
lamb2's stale list: from the subgraph, the last good list for 120 seconds.

## Consequences

A walking node answers NAMEs without a hosted subgraph, and starts with
`MARKETPLACE_SUBGRAPH_URL` blank: the mirror posture and `WORLDS_WRITES_ENABLED`
require the subgraph only where the node does not walk the index.

Authorization trails the chain head by the confirmation depth plus a pass,
and is refused for the part of each pass between walking and publishing, and
for as long as the index is behind or quarantined. A profile or an outfit on
a walking node needs the Ethereum index published even when its items are
all on Polygon. A node that does not walk the index and has no subgraph
(an island) serves profiles and outfits with no claimed name, as before.

Revisit when the walk covers the testnets, which would retire step 2.
