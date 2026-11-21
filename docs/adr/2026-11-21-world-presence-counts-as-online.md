# World presence counts as online

- Status: Accepted
- Date: 2026-11-21

## Context

Social derived online status only from Pulse's islands snapshot on `engine.islands`,
which covers Genesis. A friend or community co-member who was only inside a World
was always offline: no initial online state, no transitions, absent from
`onlyOnline` member pages and from the online audience of the community voice
started notice. The legacy social service also tracked World joins and leaves
(`peer.<id>.world.join` / `.leave`).

## Decision

Being inside a World counts as online for friends and community co-members, as in
the legacy service.

`comms::topology::Topology`, the one presence source social reads, unites two
sources, each with its own freshness:

- Pulse islands: the complete snapshot, expiring 5 s after its last receipt.
- World occupancy: the World rooms (lobby and scene rooms) of this replica's last
  complete LiveKit observation (`comms::presence`), aged from the observation's
  start and expiring after 90 s, the age at which `/live-data` and
  `/wallet/{w}/connected-world` stop answering from it.

A wallet is online while any fresh source holds it, so a wallet in both an island
and a World is online once and goes offline only when it is in neither. The
topology generation, and with it every connectivity notice, moves only when that
union changes. Only identities that are wallets count; `authoritative-server`, Cast
streamers, watchers and presentation bots (prefixed identities) never do. Genesis scene rooms do not feed it: Genesis
presence stays with Pulse.

State comes from complete reads, never from events. Every `serve` already reads
LiveKit occupancy every 30 s, so a restarted replica starts offline and recovers
on its first read, a leave LiveKit never reported is dropped by the next read,
and an unreachable LiveKit ages the World source out instead of keeping anyone
online. A read that a webhook overtook (and that admission therefore refuses to
certify) still feeds the online view; the next read corrects it.

For promptness across replicas, the replica whose webhook verified a join or
leave publishes it on `comms.room_events.v1`. Every `serve` subscribes, and a
World room event wakes its recovery for a read about 5 s later, without the
generation bump that would make an in-flight read fail. The message is a hint at
most once: a lost or unpublished one only delays the change to the 30 s read, and
a failed publish never fails the webhook.

No `peer.<id>.world.join` / `.leave` subject is published; it has no consumer in
this tree.

## Consequences

- World transitions reach every replica within about 5 to 7 s of LiveKit's
  webhook, and within the 30 s read when the hint is lost. LiveKit reports an
  unclean disconnect only after its own departure timeout.
- A player moving from Genesis into a World leaves the islands snapshot before a
  World read sees them, so friends may get an offline then an online notice about
  5 to 7 s apart (up to 30 s when the hint is lost).
- Each replica runs an extra complete read per World room event burst, coalesced
  to at most one per about 5 s. Under steady churn every replica reads LiveKit at
  that cadence instead of every 30 s.
- A wallet in a LiveKit room this node refuses to resolve is not counted as
  online, as it is not counted in occupancy.
- When complete reads keep failing (LiveKit down, or room inventory changing
  during every read), World-only wallets go offline 90 s after the last good read
  and return on the next one.
- A deployment that restricts NATS permissions grants `serve` both publish and
  subscribe on `comms.room_events.v1`.

Revisit when room occupancy moves from LiveKit to Pulse, or when a consumer needs
event-accurate World presence rather than read-based presence.
