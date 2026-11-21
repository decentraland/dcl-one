# ADR: Chain index cursor ownership and chain binding

- Status: Accepted
- Date: 2026-11-20

## Context

Each market chain index projection (an index on one network) keeps its progress in one `shop.chain_cursor` row. Its job runs under an advisory-lock lease, but cancellation is cooperative, so a pass that lost its lease can still be committing while another process starts. A cursor advance compared only the height. A rewind, a backfill lane, a fold and the publish marks compared nothing, so a late writer could still change the projection.

A row also did not record which chain its history came from. The network name `matic` can mean Polygon or Amoy depending on the node's configuration.

## Decision

- **Ownership.** The job that walks a projection claims it on the session that holds its lease: one bounded transaction raises `owner_generation` (creating the row when there is none) and the job keeps that generation for the lease acquisition. A process without leases claims on the pool.
- **Jobs.** One supervisor per network (`chain-walk-<network>`) walks the projections its process enables, each under its own lease, `<index job>-<network>` (`nft-index-matic`, `trade-events-index-matic`). A lease per index alone would not tell the networks apart, and one per network would let a process with fewer projections enabled hold a network and starve another's. Each lease acquisition gets a fresh index (no in-memory state outlives it, and a pass that panicked gets a new one), its own holdings and backoff, and at most one pass in flight in its own task; a lease request runs in a task of its own too, to its end. A projection keeps its own cadence, so one that fails, panics, waits on its endpoint or waits for its lease never holds another back; one that reached the target may join a due companion's shared walk ahead of its turn, so projections walking together go at the faster cadence. A lost lease or a stopping process ends the pass at once, even inside a fold, and the supervisor collects it before letting the lease go. A process that runs no fleet jobs takes no lease.
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
- **Shared reads.** The projections a supervisor holds that read the same endpoint and stand at the same checkpoint walk their windows together: one read of every window (an address two of them read is asked once, with the union of their first topics), each projection taking what its own filters take, committing its share with its own fenced cursor advance (nft before sales), and closing in a task of its own. Each member's own steps (its gate, its opening, its filters, its commit) run under a deadline and catch their panics; a member that overruns or panics leaves the walk and reads its cursor again with a fresh index. Each member pays an estimate of what a window would have cost it alone from its own budget (a read that narrows a capped range or halves a refused chunk may cost more, and the walk's count shows it); a shared read that fails sends each member back to its last committed checkpoint, alone, with what it has left, and one with nothing left walks alone again on a whole budget. A projection owing a backfill, asked for a rebuild, or reading beyond first topics walks alone. Every shared walk logs one summary (its members, the requests it attempted, how each part ended), also when it fails or is cut short; the members' closes count their own requests.
- **Deploys.** A binary without this step does not check the generation, and one from before the supervisors takes leases named after the index alone, which do not exclude the new ones. The process that runs the indexes is stopped before a newer one starts, and before a rollback.

## Consequences

- Every index checks the source's chain each pass (one `eth_chainId`); before, only trade events, committee and rentals did. A source of another chain fails only that projection's pass, which backs off on its own.
- A control operation whose statement the database refuses (a lock or statement timeout) fails alone. An error that ends the session (FATAL or PANIC, a broken connection, the keepalive bound) drops every lease of the process at once.
- The cursor row lock is held only for the commit that takes it, not for a fold's duration. Writers that also move the notification producers' cursors move them first.

## Validation

- The market index tests cover a job that lost its projection at the same block and hash, which commits no window, rewind, lane, fold, fold record, publish, withdrawal or digest batch.
- They also cover a legacy cursor adopted by proof or set apart, a backend that does not answer, revalidation, a rebuild that keeps the other network's rows and converges, and a job pass that refuses another chain and claims again after a fence.
- They cover a trade-events rebuild that recomputes a lost V3 digest, a rebuild refused across chains, a rebuild refused on an unproven history (rows and producer positions kept), a rebuilt cursor that stays bound against a chain switch before its replay, and verdict SQL that ignores a cursor bound to another chain than the served one.
- Notice producer tests cover a trade source read on the configured chain and transfer notices that refuse an incompatible pair; a tips test covers a quarantined MANA cursor that evicts no claim.
- The supervisor tests cover a projection that keeps its cadence while others on its network fail, wait on a silent endpoint, wait for their lease or panic (with a cleanup that would panic too); a stop that lets a lease request in flight finish and asks for no other; an idle projection whose lost lease resets its backoff; a lost lease that interrupts a fold before it commits, and a successor that claims once and publishes; processes with different projections enabled sharing a network with one writer per projection; both networks handed over between processes; and a process without fleet jobs that takes no lease, reads no chain and claims nothing.
- The jobs tests cover control operations on the lease session per acquisition, a control of a released acquisition that never runs once the name is taken again, and overlapping acquisitions of one name that get one guard. Builder and badges tests cover unbound and quarantined cursors.
- The pinned request-order test is unchanged.
