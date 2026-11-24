# Marketplace sales accounting

## Context

A marketplace transaction can fill several orders, redirect payments, or exchange several assets for one amount. A collection store emits an issue for each minted NFT and a purchase event listing its line prices. A block reorganization can remove a sale and replace it with a sale for a different account, item or day.

## Decision

The shop records one execution at the venue event and keeps its asset and payment legs in venue order. Store line prices come from the purchase event. Matching an issue supplies the minted token. For an execution with a single asset and a single consideration currency the asset has a price. For bundles without line prices the consideration stays with the execution. NAME registration fees have a separate execution kind.

Amounts are integers in the payment token's base units. Receipt transfers and fee terms at the execution position determine payee roles. The payer, NFT beneficiary, asset seller and payment beneficiary occupy separate fields. Downstream accounting can distinguish an unknown split from a zero fee.

The trade, NFT and sales projections keep independent ownership of their logs. A per-network accounting fold identifies the common prefix covered by all three. Publication records its block hash, decoder version and source revisions. A source repair can change a revision at the same height. A fold locks and checks all three input cursors before making its results visible. Readers check the publications for all requested networks in the transaction they use for the data.

## Consequences

Independent forward progress preserves a recent, proven prefix. A rewind, changed historical coverage or expired publication makes affected reads unavailable. Rollups retain their currency, UTC day and underlying execution identities so a rebuild can remove groups emptied by a reorganization.
