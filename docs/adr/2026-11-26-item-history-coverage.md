# ADR: Item history coverage proves an unused item URN

- Status: Proposed
- Date: 2026-11-26

## Context

The builder must refuse a third-party item URN that a wearable or emote
already holds on the catalysts this node syncs from, including deployments
the node hasn't synced yet. On a node that syncs from peers, a row missing
from `content.deployments` proves nothing: the incremental sync can leave
entities behind its cursors and processed snapshots.

## Decision

- **A replay proves coverage.** The process that holds the `sync` lease
  replays every peer's wearable and emote `/pointer-changes` history from
  `SYNC_FROM_TIMESTAMP_MS`, in passes, up to `SYNC_ITEM_REPLAY_CONCURRENCY`
  peers at once (default 8), each from its own checkpoint, and deploys what
  the node lacks through the sync deployer. A pass counts as debt every
  delta it can't prove accepted: an entity still missing after the drain, a
  record that doesn't parse, an implausible timestamp, a page that doesn't
  advance the keyset, and a peer whose page keeps failing past its retries.
- **The proof names its writer.** A completed pass records its scope
  (algorithm, sorted peers, item types, history floor), its start, its debt
  and the lease session that wrote it (pid and backend start). Every write
  is one statement conditioned on that session holding the `sync` advisory
  lock.
- **The reader fails closed.**
  `content::sync::item_coverage::item_history_coverage` answers
  `Covered { as_of }` only when the record's algorithm and item types match,
  its floor and debt are 0, the live lease holder is the session that wrote
  it, and the pass started at most 30 minutes ago. Anything else is
  `Unknown` with its reason. Coverage proves absence from the peers as they
  were when the pass started, never global absence.
- **Own content needs no proof.** A process that serves unsynced with no
  `SYNC_SOURCE` holds all of its content
  (`SyncStatus::is_own_content_authority`).
- **The builder takes an unused URN only with the proof.** A third-party
  URN that no item and no active entity holds is free only when the node is
  its own content authority or both item types are covered, read in the
  same transaction before the content. Otherwise the request answers 503.
- **Passes stay full.** Each pass rereads the whole history and starts
  `SYNC_ITEM_REPLAY_INTERVAL_SECS` after the previous one started (default
  1200; 0 turns the replay off).

## Consequences

- Each pass reads about 81 MB in 160 requests from each source (measured on
  November 20, 2026), so the default costs about 5.8 GB a day per source
  catalyst.
- A pass lasts about as long as each of its readers' share of the source
  walks, about 2 minutes a walk at the default page pause. Coverage stays
  continuous while the larger of the interval and a pass, plus a pass,
  fits in 30 minutes: up to five walks per reader (40 sources) at the
  default interval, page pause and concurrency. A longer pass logs a
  warning, and for part of every cycle readers get `Stale` and URN checks
  answer 503.
- With the replay off, or before its first pass completes, a URN check that
  needs the proof answers 503.
- The sync leader and every reader connect as the same database role, or
  the readers' role holds `pg_read_all_stats`, and the node reaches
  PostgreSQL directly or through a pooler in session mode.

## Alternatives

- Passes that read only past the previous pass's newest `localTimestamp`:
  cheaper, but a deployment a peer commits late with an older timestamp
  would stay hidden under a fresh proof.
- Reading the peers one after another from one checkpoint: a pass lasts
  the sum of the sources' walks, so coverage lapses beyond about five
  sources.
