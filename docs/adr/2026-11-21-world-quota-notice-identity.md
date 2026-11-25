# ADR: World quota notices retain the first notice for each creation day

- Status: Accepted
- Date: 2026-11-21

## Context

World storage quota changes produce a missing-resources warning, an access
restriction and an access-restored notice. The block's creation time anchors
the grace deadline. A block can be lifted and created again on the same day,
and notice writes can repeat after retries or restarts. These repeats must not
replace the first notice or create another email delivery.

The producer, Social's directory and the shared notification writer use the
same PostgreSQL transaction. Existing notice rows can have IDs derived from a
full block-creation timestamp or imported IDs. Preserving those rows also
preserves their read state and email delivery records.

## Decision

Quota notice identity is the normalized wallet, notice kind and UTC calendar
date when the block was created. The three kinds remain independent.
Restoration uses the block's creation date even when restoration occurs on a
later date. A new notice uses a deterministic daily UUID.

The producer states the quota persistence policy explicitly. Social passes
that policy to the one writer in `shared::notifications`; it does not infer
quota behavior from a separate list of notice-kind strings. Other producers
retain their existing metadata-refresh behavior.

Social sorts quota notices by wallet. For each notice, the shared writer takes
that wallet's transaction-scoped notification lock before aliasing and
inserting. Its namespace is separate from Worlds' quota-enforcement locks.
The locks remain held until the mutation transaction commits or rolls back.

Whenever any quota kind is emitted with an exact block-creation timestamp, the
writer links existing native notices for all three kinds to their corresponding
daily keys through `social.notifications.broadcast_key` where no daily carrier
already exists. This includes a restoration emitted before another detection
pass. A linked row retains its ID, contents, read state and delivery records.
Otherwise the writer inserts
the current notice under its daily ID and key. A repeated daily key leaves the
first notice unchanged and never queues another email delivery.

This uses the existing notification primary key and unique
`(address, broadcast_key)` index. It adds no table, index, dependency, process
or internal network request. A failure rolls aliasing and insertion back with
the quota change.

## Consequences and cutover boundary

Two blocks created on the same UTC day share each kind's first notice, even
if their grace deadlines differ. Warning, restriction and restoration remain
separate notices. A different wallet or block-creation day receives a distinct
notice. Quota enforcement, grace duration, access decisions and the `when`
deadline calculation do not change.

Exact native aliases cover a block whose creation timestamp remains available.
Already closed native blocks and imported history can lack the facts needed
to link their old rows. That history may coexist with one additional daily
notice per wallet and kind at cutover; subsequent repeats share the daily key.
This accepted boundary does not promise consolidation of every historical
duplicate, infer missing creation dates or rewrite existing IDs.

Daily uniqueness requires a coordinated rollout: retire quota writers that do
not set daily keys before relying on this rule. Writers using only full
creation timestamps can otherwise insert unkeyed notices beside daily rows.

## Verification

Database-backed checks cover same-day block/lift/re-block, repeated delivery
after an email is sent, a later UTC creation day, independent wallets and
kinds, and restoration on a later date. Upgrade checks cover all three exact
native aliases, restoration before redetection, rollback and concurrent
writers. Assertions inspect stored metadata, occurrence time, read state and
delivery state through the production directory and shared writer.
