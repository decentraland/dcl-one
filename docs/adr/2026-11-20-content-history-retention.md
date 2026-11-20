# ADR: Content history retention

- Status: Accepted
- Date: 2026-11-20

## Context

The content server differs from the legacy catalyst
([`decentraland/catalyst@6c56f8a3`](https://github.com/decentraland/catalyst/tree/6c56f8a3eaff631cb7be0dc01ef92a24333bbc93))
in three retention behaviors: it prunes superseded deployments, it does not
expire profiles, and it never retries a validation refusal.

## Decision

The content server keeps these three retention behaviors as they are.

### Superseded deployments are pruned

When `CONTENT_HISTORY_PRUNE_ENABLED` is on, the history prune
(`services/catalyst/content/src/history_prune.rs`) deletes a deployment no
pointer serves any more: one superseded by a newer entity on the same pointers,
or one that was unpublished. It deletes the row, its `content_files` rows and the
blobs nothing else names. A row becomes eligible once it has been out of
`active_pointers` for `CONTENT_HISTORY_PRUNE_GRACE_HOURS` (24 by default). The
switch is off in the binary and on under the mirror posture. It is independent
of `CONTENT_WRITES_ENABLED`.

The prune never touches a scene this node deployed (`local_entities`), the rows
that scene superseded, the files of `GLOBAL_SCENES_URN` scenes, or
`failed_deployments`, `processed_snapshots` and the sync cursors.

Once a row is pruned, `/audit`, `/deployments` and `/pointer-changes` stop
returning it. On a node with pruning on, those routes cover active entities plus
roughly the last day of superseded ones. The legacy garbage collector deletes
only the blobs of overwritten deployments and keeps the rows, so its history
routes list every superseded entity, even after its files are gone.

#### Overtaken retries are discarded

With history pruning on, the retry worker
(`services/catalyst/content/src/sync/retry_failed.rs`) discards a retried entity
that a newer active entity of its type has overtaken on any of its pointers: it
deletes the entity's failure record and writes no deployment row. Pointer
succession supersedes an entity as a whole, so such an entity could only land as
a superseded row, and the prune would remove that row again. The worker checks
this after it fetches the entity file and before it downloads content or
validates, and only when that file hashes to the entity id. It does not discard
an entity when a newer deployment of this node's own (`local_entities`) of its
type shares one of its pointers, since the prune keeps the rows such a scene
superseded. With pruning off, the entity is deployed as a superseded row.

### Profiles do not expire

Every profile stays active until a newer profile on the same pointer replaces
it. There is no counterpart of the legacy `PROFILE_DURATION` expiry, which stops
serving profiles older than a year (except `default*` pointers) on every node,
and also deletes them when garbage collection is on. A dcl-one node therefore
serves profiles that a legacy peer no longer serves.

Peer sync still skips profiles older than `SYNC_PROFILE_MAX_AGE_DAYS` (365 by
default; 0 syncs every profile), so a node never imports such a profile from
snapshots or the pointer-changes feed. Read-through does not apply that age, so
an old profile a reader asks for is still fetched.

### Validation refusals are final, and failed records expire

The retry worker (`services/catalyst/content/src/sync/retry_failed.rs`) treats a
failed sync deployment according to why it failed:

| reason | retried | dropped |
| --- | --- | --- |
| deployment error, missing entity | with backoff (15 minutes doubling to a day), up to `MAX_FAILED_DEPLOYMENT_RETRIES` (10) | at the retry cap, or by the daily prune |
| validation undecided (a check could not be decided, such as an unavailable RPC) | with a wait that grows from 15 minutes to a day, without spending retries | becomes a refusal after 7 days undecided |
| deployment rejected (validation refused it) | never | by the daily prune |

The daily prune deletes every record whose failure time is older than
`RETRY_FAILED_PRUNE_TTL_DAYS` (7), except undecided records. The failure time is
the time of the latest failure that sync, read-through or a failed database
write recorded, or of the retry worker giving up on an undecided record; a retry
that fails again keeps it.

The legacy catalyst retries every failure kind, refusals included, up to the
same cap. It has no age-based prune. A refusal caused by a rule or chain-index
problem that is later fixed can still deploy there on a later retry. Here it
stays undeployed.

## Reasons

- Peer sync stays correct without history rows. Its check for whether an entity
  is already known reads only deployment rows (`is_entity_deployed`, and
  `deployed_entity_ids` for a snapshot batch), so a pruned entity a peer offers
  again is downloaded again and lands as a superseded row. Pointer
  succession never moves a pointer back to an older entity, so a re-offer costs a
  download, never a wrong pointer. The grace period covers what a restart can
  replay from snapshots and the pointer-changes feed.
- Keeping only active entities and a short tail bounds the database and the blob
  store by the active set rather than by every version ever deployed. The legacy
  approach bounds only the blobs.
- Expiring profiles takes avatars away from players who have not redeployed in a
  year. Keeping them costs only their rows and images.
- Retrying a refusal repeats a check that already gave a definite answer.
  Undecided checks are the transient case, and they get a week of retries.

## Consequences

- Consumers that read the full deployment history of an entity or pointer through
  `/audit`, `/deployments` or `/pointer-changes` must read it from a node with
  pruning off. A node operated as a history source keeps
  `CONTENT_HISTORY_PRUNE_ENABLED=false`.
- `/entities/active` and `/profiles` answers for long-inactive profiles differ
  from a legacy peer's.
- After a validation rule or chain-index fix, refused entities do not deploy on
  their own, and after 7 days their records are gone, so the node has no list of
  them to re-run.

Revisit this decision if a required consumer needs the full history from every
node, if profile expiry becomes a network rule, or if refusals turn out to be
transient often enough to need retries.
