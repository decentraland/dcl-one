# World presence counts as online

- Status: Accepted
- Date: 2026-11-21

## Context

Social derived online status only from Pulse's islands snapshot on `engine.islands`,
which covers Genesis. A friend or community co-member who was only inside a World
was always offline: no initial online state, no transitions, absent from
`onlyOnline` member pages and from the online audience of the community voice
started notice. The legacy social service also tracked World joins and leaves
(`peer.<id>.world.join` / `.leave`) as they happened.

## Decision

Being inside a World counts as online for friends and community co-members, and
World joins and leaves take effect as they happen, as in the legacy service.

`comms::topology::Topology`, the one presence source social reads, unites Pulse
islands (expiring 5 s after the last receipt) with a World presence source. A wallet
is online while either holds it, so a wallet in both an island and a World is online
once and goes offline only when it is in neither. The topology generation, and with
it every connectivity notice, moves only when that union changes.

**Events.** The World presence source holds LiveKit participant sessions in World
rooms (lobby and scene rooms), keyed by participant sid. The replica whose webhook
verifies a `participant_joined` or `participant_left` applies it at once and
publishes it on `comms.room_events.v1`; every `serve` applies what it receives. This
includes a leave the webhook's own session ordering supersedes, such as an old
session's late leave. Only a room whose LiveKit name is its canonical name is
applied and published, the same rooms a complete read counts; its events still
reach the access log and badges.

- A sid joins at most once, so an event leave is final whatever order the two arrive
  in. A join that arrives after its leave (or is redelivered) never brings the player
  back, and a duplicate changes nothing. Event leaves are remembered for 5 minutes,
  at most 200,000 of them, the oldest forgotten first.
- Each wallet keeps a count of its live sessions; only its first session and its
  last leave change the published set, so other events cost no set work.
- A player in several World rooms has one session per room and stays online until
  the last one leaves. A reconnect is a new sid, so the old session's late leave
  does not take the player offline.
- Only identities that are wallets count. `authoritative-server`, Cast streamers,
  watchers and presentation bots (prefixed identities) never do. Genesis scene
  rooms do not feed this source: Genesis presence stays with Pulse.

**Complete reads correct drift.** Every `serve` already reads the complete LiveKit
occupancy every 30 s. A read records the instant it started, before enumerating
rooms, and then:

- confirms the sessions it lists and adds any it finds that no event announced
  (missed events, restarts);
- ends live sessions it does not list, unless an event changed them after the read
  started, so a newer event always wins. A session a read ended is forgotten, not
  final: a later read or join that lists it brings it back;
- never revives a session whose leave an event applied.

A read that a webhook overtook still corrects drift this way, even though admission
refuses to certify it. A read that fails after listing rooms for any reason but
LiveKit itself (changed inventory, budget, deadline) confirms and adds the sessions
it listed and ends none.

**Bound.** A session that no event or read has confirmed for 90 s stops counting;
sessions expire from a deadline-ordered queue.
That is the age at which `/live-data` and `/wallet/{w}/connected-world` stop
answering from the read. While reads succeed, they confirm every present session
every 30 s. When they keep failing, a fresh join still counts for its first 90 s,
and an unconfirmed session ends at most 90 s after its last confirmation, never
"online forever". Leaves keep applying at once either way.

Receiving an event wakes no read: the events themselves keep every replica current,
and the 30 s read is only the correction. The webhook replica's earlier re-read
after its own webhook is the existing admission invalidation and is unchanged. No
`peer.<id>.world.join` / `.leave` subject is published, because no consumer of it
exists in this tree.

## Consequences

- World joins and leaves reach every replica as soon as the webhook is verified and
  the NATS message delivered. LiveKit reports an unclean disconnect only after its
  own departure timeout.
- A lost NATS message leaves one replica wrong until its next complete read, at most
  about 30 s later.
- A player moving from Genesis into a World can leave the islands snapshot shortly
  before the World join arrives, so friends may briefly get offline then online.
- Memory is bounded: live sessions follow live participants up to 200,000, and event
  leaves are kept for 5 minutes up to 200,000. A join past the live cap is dropped;
  drops are counted in `comms_world_presence_dropped_total{reason}`.
- `comms_presence_recovery_passes_total{result}` gives the complete read failure rate,
  and `comms_world_presence_sessions` / `comms_world_presence_wallets` the World size.
- A wallet in a LiveKit room this node refuses to resolve produces no event and no
  read entry, so it is not counted as online.
- If LiveKit stays unreachable, players who joined more than 90 s earlier go offline
  until a read succeeds. A read that fails after listing still confirms what it
  listed; past the read budget (4,096 rooms or 100,000 participants) the rooms it
  did not reach go unconfirmed.
- This is the first publisher on `comms.room_events.v1`, Genesis scene rooms
  included. Any NATS client allowed to publish on it can set World presence, the
  same trust as `engine.islands` for island presence.
- A deployment that restricts NATS permissions grants `serve` both publish and
  subscribe on `comms.room_events.v1`.

Revisit when room occupancy moves from LiveKit to Pulse.
