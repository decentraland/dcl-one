# Mainnet catalogue mirror proof and RPC outage fallback

- Status: Accepted
- Date: 2026-11-25

## Context

Mainnet catalogue mirrors previously read their configured graphs without an
RPC. Proving a pinned block catches a source on the wrong chain and keeps
page reads on one snapshot. Mainnet prices also need a fresh completed item
pass, with a budget of four item intervals and a minimum of 30 minutes.

## Decision

For mainnet with a chain-bound RPC, prove a block at the usual confirmation
depth below the lower of the graph and RPC heads. Keep serving the last
completed copy while proof is unavailable.

The proof reads that block's hash from the RPC and asks the graph for the
block by that hash. graph-node answers a `_meta` pinned by block number with
a null hash, so the graph cannot supply the hash itself. A graph that holds
no block with that hash, or holds it at another height, is a hash
disagreement.

Limit failed-pass retry delays to five minutes. Once RPC failures have
persisted for ten minutes, let the next pass refresh from the configured
graph without a pin. The next new pass attempts proof again. At the default
five-minute item interval, the normal interval, retry window and maximum retry
delay together fit inside the price freshness budget. Price freshness
requirements continue to apply during a graph outage.

A graph-side proof error neither starts nor resets the RPC outage clock.
A wrong-chain result or a same-height hash disagreement bars fallback
until a later proof succeeds. Mainnet passes with a stored pin can finish
on a replacement writer without an RPC. Testnet readers rely on a proven
copy and require the testnet proof RPC.

See [DATABASE.md](../../DATABASE.md) for the catalogue pair and rollout
contracts and [METRICS.md](../../METRICS.md) for proof outcomes.
