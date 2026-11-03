# ADR: Deploy access read from live L1 and L2 RPCs

- Status: Proposed
- Date: 2026-11-03

## Context

A content deploy is accepted only when its signer may deploy it: LAND rights for a scene,
names and worn items for a profile or outfits, collection rights for a wearable or emote, an
approved merkle root for a third-party item. The reference catalyst answers these questions
with its on-chain validator: `eth_call`s to the Checker contract of `@dcl/catalyst-contracts`
and to the item contracts, at the block of the entity timestamp and at the block five minutes
before it. Indexes and subgraphs lag the chain and can disagree with it at a given block, so a
node that reads them answers differently from the reference for recent and historical entities.

## Decision

The content validator asks the chain the way the reference does, over `ETH_RPC_URL` (L1) and
`RPC_ENDPOINT_POLYGON` (L2), with the contracts vendored for the `ETH_NETWORK`
(mainnet with polygon, sepolia with amoy). Blocks come from a binary search over the RPC that
remembers the blocks it read.

- LAND is granted per parcel at either block. Names, and the items of each chain, must all pass
  at one block.
- A check the RPC cannot answer, or an entity whose checks together run past 30 s, is undecided:
  the deploy gets 503 with `Retry-After`, never a grant. A call the chain reverts is a refusal.
- Item ownership calls and the probes of third-party NFT standards go out as JSON-RPC batches,
  with caps on the distinct items (200) and third-party contracts (32) asked per check; a
  contract of no known standard is not probed again for five minutes.
- A node with `CONTENT_WRITES_ENABLED` refuses to start without both https RPCs and a known
  network, unless `IGNORE_BLOCKCHAIN_ACCESS_CHECKS` is set; an RPC answering another chain id is
  refused. The journey posture sets `IGNORE_BLOCKCHAIN_ACCESS_CHECKS`, since it reads no chain.
- The escrow leases in `shop.usage_grants` still count as ownership of the leased item.

## Consequences

- Local deploys depend on two external RPCs being up and able to answer for past blocks; an
  outage or a pruned node turns deploys into 503s until it recovers. Operators should point
  both variables at archive-capable endpoints.
- Each deploy costs a few RPC calls: two block searches per chain when the timestamp is new,
  then one Checker multicall or one item batch per block, plus two probe batches the first time
  a third-party contract is seen.
- The LAND index stays for worlds, comms and the unpublish route; it no longer decides deploys.
