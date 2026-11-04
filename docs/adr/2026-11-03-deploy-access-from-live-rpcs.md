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

- Every node has both RPCs, as the reference does with `L1_HTTP_PROVIDER_URL` and
  `L2_HTTP_PROVIDER_URL`: left unset or empty, they are
  `https://rpc.decentraland.org/{mainnet,polygon}?project=catalyst-content` on mainnet and
  `.../{sepolia,amoy}?project=catalyst-content` on any other network. The island and journey
  postures dial no outside host, so they get no default.

- LAND is granted per parcel at either block. Names, and the items of each chain, must all pass
  at one block.
- A check the RPC cannot answer, or an entity whose validation runs past 30 s from its
  signature step to the end of its access checks, is undecided: the deploy gets 503 with
  `Retry-After`, never a grant. A call the chain reverts is a refusal. The deadline bounds the
  validation, not the whole request (the upload and the store write come around it).
- Item ownership calls and the probes of third-party NFT standards go out as JSON-RPC batches.
  One check asks about at most 200 distinct items from at most 200 third-party contracts; an
  entity naming more is refused as too many to check, never reported as not owned. A contract
  that holds code but answers no known standard is not probed again for five minutes; an
  address without code is probed again on the next check.
- A node with `CONTENT_WRITES_ENABLED` refuses to start without both https RPCs and a known
  network, unless `IGNORE_BLOCKCHAIN_ACCESS_CHECKS` is set; an RPC answering another chain id is
  refused. Outside island and journey the defaults satisfy this. The journey posture sets
  `IGNORE_BLOCKCHAIN_ACCESS_CHECKS`, since it reads no chain; an island taking deploys needs it
  too, or loopback https RPCs.
- The escrow leases in `shop.usage_grants` still count as ownership of the leased item.

## Consequences

- Local deploys depend on two external RPCs being up and able to answer for past blocks; an
  outage or a pruned node turns deploys into 503s until it recovers. The defaults are
  Decentraland's public RPCs; operators can point both variables at their own archive-capable
  endpoints.
- With the RPCs always present, the readers that answered empty or skipped a check without them
  now ask the chain: third-party ownership and the relay's collection-code and meta-transaction
  signature checks. The default L1 RPC alone turns on:
  - EIP-1654 contract-wallet signature verification, for explore, social and deploys;
  - `/lambdas/contracts/servers` and `/lambdas/contracts/denylisted-names`
    (`/lambdas/contracts/pois` reads the L2 RPC);
  - the `executions-track` poller of MANA executions, on mainnet only;
  - the NAMEs Ethereum signer and its receipt reconciler, when `META_TX_BROADCAST_ENABLED=true`
    and `RELAYER_PRIVATE_KEY_FILE` are set; its routes still need the admin token and
    `DATA_WRITES_ENABLED`, and a set `NAMES_MAX_PRICE_WEI` caps each buy.

  The chain indexes keep their own `*_ENABLED` switches.
- An `ETH_RPC_URL_FILE` or `RPC_ENDPOINT_POLYGON_FILE` naming an empty file fails startup
  rather than reading the default; each default applied is logged by variable name.
- Each deploy costs a few RPC calls: two block searches per chain when the timestamp is new,
  then one Checker multicall or one item batch per block, plus two probe batches the first time
  a third-party contract is seen.
- The LAND index stays for worlds, comms and the unpublish route; it no longer decides deploys.
