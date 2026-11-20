# ADR: Chain index cursor ownership and chain binding

- Status: Accepted
- Date: 2026-11-20

## Context

Each market chain index projection (an index on one network) keeps its progress in one `shop.chain_cursor` row. Its job runs under an advisory-lock lease, but cancellation is cooperative, so a pass that lost its lease can still be committing while another process starts. A cursor advance compared only the height. A rewind, a backfill lane, a fold and the publish marks compared nothing, so a late writer could still change the projection.

A row also did not record which chain its history came from. The network name `matic` can mean Polygon or Amoy depending on the node's configuration.

## Decision

- **Ownership.** The job that walks a projection claims it on the session that holds its lease: one bounded transaction raises `owner_generation` (creating the row when there is none) and the job keeps that generation for the lease acquisition. A process without leases claims on the pool.
- **Fencing.** Every transaction that writes a projection ends by locking its cursor row and comparing that generation, the chain binding and the quarantine; on a mismatch it rolls back. This covers:
  - the window commit, which also requires the checkpoint it read (height and hash);
  - a rewind;
  - the nft backfill lane;
  - every fold, including each trade-events digest batch;
  - the fold record, the publish and its withdrawal.

  A fenced job drops its generation, and its next pass claims again, which only succeeds while it still holds the lease.
- **Binding.** `chain_id` records the chain the history was read from. The source's chain is checked every pass. A row from before the binding is adopted only when it has no history or when its stored last-block hash equals the source's at that height. Adoption is a compare-and-set on the generation and the checkpoint.
- **Quarantine.** A history that cannot be proven is set apart, with a reason, and its publication withdrawn: a differing hash, no stored hash, or a binding to another chain. A source that does not answer decides nothing. A row in quarantine is not walked, rewound or published.
- **Readiness.** Readers require the binding to the chain they expect and no quarantine besides the existing rule. Indexes that read mainnet only expect chain 1 or 137. The readers of `trade_events` expect the configured chain: the Rust readers and the notification producers pass it, and its verdict fragments in the listing SQL read the chains the process serves. The transfer notices match two indexes at the same heights, so they run only when both read Polygon mainnet.
- **Operator procedures.** Revalidate (keep the history, prove it again) and rebuild (the owner runs the projection's own cleanup to its start block in a fenced transaction; trade events also drop their derived verdict rows). Each is one `UPDATE` that also raises the generation; `DATABASE.md` "Market chain index cursors" spells them out. A rebuild keeps the projection's chain and start block, and the rebuilt cursor stays bound to the chain it proved. It is refused, with the history kept, in three cases: across chains, across starts, or on a history with no binding whose checkpoint the source does not prove. A database for another chain begins empty.
- **Deploys.** A binary without this step does not check the generation. The process that runs the indexes is stopped before one with the step starts, and before a rollback to one without it.

## Consequences

- Every index checks the source's chain each pass (one `eth_chainId`); before, only trade events, committee and rentals did. In a job that walks both networks, a source of another chain skips that network only; the job fails, and backs off, only when no network walked.
- A control operation whose statement the database refuses (a lock or statement timeout) fails alone. An error that ends the session (FATAL or PANIC, a broken connection, the keepalive bound) drops every lease of the process at once.
- The cursor row lock is held only for the commit that takes it, not for a fold's duration. Writers that also move the notification producers' cursors move them first.

## Validation

- The market index tests cover a job that lost its projection at the same block and hash, which commits no window, rewind, lane, fold, fold record, publish, withdrawal or digest batch.
- They also cover a legacy cursor adopted by proof or set apart, a backend that does not answer, revalidation, a rebuild that keeps the other network's rows and converges, and a job pass that refuses another chain and claims again after a fence.
- They cover a trade-events rebuild that recomputes a lost V3 digest, a rebuild refused across chains, a rebuild refused on an unproven history (rows and producer positions kept), a rebuilt cursor that stays bound against a chain switch before its replay, and verdict SQL that ignores a cursor bound to another chain than the served one.
- Notice producer tests cover a trade source read on the configured chain and transfer notices that refuse an incompatible pair; a tips test covers a quarantined MANA cursor that evicts no claim.
- The jobs tests cover control operations on the lease session per acquisition, a control of a released acquisition that never runs once the name is taken again, and overlapping acquisitions of one name that get one guard. Builder and badges tests cover unbound and quarantined cursors.
- The pinned request-order test is unchanged.
