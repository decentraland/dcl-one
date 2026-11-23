# ADR: Worlds change feed

- Status: Accepted
- Date: 2026-11-23

## Context

The worlds content server
([`decentraland/worlds-content-server@ebbf8d4`](https://github.com/decentraland/worlds-content-server/tree/ebbf8d4))
publishes an SNS event for every world publish, undeploy and settings, spawn
or access change. Asset-bundle conversion, places, world storage and
multiplayer consume them. A node publishes no SNS
([worlds compatibility](../services/catalyst/docs/worlds-compatibility.md)),
so worlds published on a node never reach those consumers.

## Decision

The node records the events and a relay outside it publishes them.

- `worlds.change_log` holds, per change made on this node, the upstream
  message body and its message attributes besides type and subType. The
  write path inserts it in the transaction of the change, so a row exists
  exactly when the change committed. The worlds mirror, peer sync and the TS
  import never write it: the feed carries only this node's own changes.
- Bodies are the upstream's exactly (the `@dcl/schemas` event schemas forbid
  additional properties). A deployment names `WORLDS_PUBLIC_URL`, by default
  `<PUBLIC_URL>/worlds-content-server`, as its content server. The
  `timestamp` is the database clock read after the world's row lock, so one
  world's events are timestamped in commit order; a relay passes it on.
- `GET /worlds-content-server/changes?after=<txid>:<id>&limit=<n>` pages the
  rows in `(txid, id)` order, only rows of transactions older than every
  transaction still running. A transaction that took its id first and
  commits last is therefore never skipped by a reader's cursor. Rows are kept
  30 days.
- The route requires its own bearer, `WORLDS_CHANGE_FEED_TOKEN`
  (`Authorization: Bearer <token>`, compared in constant time; unset refuses
  every request with 401). It grants nothing else, and the worlds admin token
  does not open it: a deployment event carries the deploy's auth chain, which
  no public worlds route serves.

Out of scope: access allow-list, permission and block events, which the node
handles itself.

## Consequences

- A relay holds only the feed token, which reads the feed and nothing else.
- The feed lags the cluster's oldest open write transaction.
- Transaction ids do not survive a logical dump and restore: after one, the
  table is emptied and readers start again from its oldest row.

## Rejected

- Publishing to SNS from the node: it ties every node to one cloud account
  and makes delivery part of the write path.
- Diffing the public world listings: it cannot tell local from mirrored
  worlds, and it sees removals only as absences.
- An id-only cursor: a lower id whose transaction commits after a reader
  passed it would be skipped.
