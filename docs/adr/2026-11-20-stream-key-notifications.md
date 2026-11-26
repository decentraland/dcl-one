# ADR: The stream key check wakes on a database notification

- Status: Accepted
- Date: 2026-11-20

## Context

A stream key ends when its minter loses the place or the scene of its room is replaced ([decision](2026-11-20-stream-key-follows-authority.md)). The check ran on each use of the key and in a periodic job, so a redeploy or a world permission change left the key and its stream alive until the next pass. The writes that change authority happen in other areas: content inserts deployments, worlds changes scenes, owners and permissions. Those areas do not call comms, and a shorter job period still waits for its pass.

## Decision

The gatekeeper listens on two PostgreSQL channels and checks the keys of the place a message names.

- `new_deployment`, the channel content already publishes for each deployment and scene removal. The listener keeps the scene messages and reads `content.deployments` by shared SQL.
- `world_authority`, a new channel. The schema function `worlds.notify_world_authority` and its triggers publish `world:<name>` when a world's local scenes or owner change, or when a permission is removed or narrowed.
- The listener is one leased job, so one `serve` process runs it. It holds one connection of the serve pool, keeps the waiting payloads as a set, so the messages for one place that arrive during a check are one more check, and checks every active key after each subscription. After a minute with no message it runs one statement with a 10 s limit: a session that ended fails there, and the listener subscribes again.
- A payload names a public place and only selects keys for the check. The check reads the authority again.
- The periodic key recheck job stays for lost messages and for the changes that have no message: a LAND owner or operator change, a NAME transfer before the owner reconcile, a ban.

## Consequences

A redeploy or a world authority change ends a stale key in less than a second on a node with a live listener. The trigger function is part of the bootstrap schema; `catalyst schema migrate` creates it on an existing database, and no step exists. `DATABASE.md` "Stream key notifications" has the workload and the invariants, and changes to a channel, a payload, a trigger or a column the listener reads update it in the same change.
