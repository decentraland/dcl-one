# One Marketplace chain pair per catalogue database

## Decision

A catalogue database serves Ethereum and Polygon mainnet (1/137), or
Sepolia and Amoy (11155111/80002). Catalogue rows and cursors key by
network family. Keeping one pair per database lets the readers, indexers
and notification producers use the same keys. Ethereum and Sepolia
Rentals histories have separate keys and can coexist.

The index process establishes the pair in `shop.worker_progress` before
starting a catalogue writer. It checks the pair against `ETH_NETWORK`
and the stored cursors and mirror bindings. API replicas read the configured
pair without claiming the binding. Walking readers require fresh,
chain-bound publications in the same snapshot as their data. While a
source catches up, the route answers its unavailable envelope.
Non-walking nodes keep their configured subgraphs.

## Operations

Provision a distinct database for the other pair. The eligible legacy
testnet mirror copies have a separate, explicit rebind procedure. See
[DATABASE.md](../../DATABASE.md) for rebind, migration and warm-up steps.

[The testnet catalogue and reader changes](https://github.com/decentraland/dcl-one/issues/1693)
