# comms-e2e

End-to-end scenarios against a running comms stack: the archipelago connector, the comms
gatekeeper, LiveKit and Pulse. A scenario scripts a situation, states what must hold, and fails
with a timeline of what every actor sent and saw when it does not.

It exists because the bugs this stack has had were rarely about load. They needed two to twenty
actors with exact timing: the same wallet connecting twice, a reconnect racing a stale socket, a
link that stalls for two seconds. Unit tests and code review did not reproduce them; a person
running two clients did.

## Parts

| Part | What it is |
|---|---|
| Protocol bots (`pulse`, `archipelago`, `gatekeeper`, `livekit`) | Thin clients. They sign, connect and send whatever a scenario tells them to, valid or not, and keep a view of what the server told them. They have no policy of their own: no reconnect, no pacing, no backoff. |
| Fault link (`fault`) | A relay a client dials instead of the stack. A scenario can stall, drop, delay, reorder or duplicate either direction, give it a path with a delay, jitter that never reorders and loss (`Link`), queue what arrives and let it go together (`Queue`), half-open or cut the flows that exist, orphan TCP connections (closed towards the client, left open towards the stack) and close a UDP port so it answers "unreachable". A UDP link notes every datagram it receives, in time order, before its fault acts: its time, its size and what the fault did with it, or that the relay dropped it first (a closed port, a half-open or cut flow), as lost (`history`, `windows`, `cadence`). What a queue kept for a flow half-opened or cut before the queue ended is lost with that flow. |
| Explorer actor (`explorer`) | The real headless explorer as a child process, behind fault links, driven through its loopback console. Bots are its observers. |
| Oracles (`oracle`, `livekit::RoomService`) | The servers' own word: Prometheus metrics, the statistics routes, LiveKit's room service. |
| Engine (`engine`) | Runs scenarios, repeats races with seeds, waits on conditions with deadlines, and reports. |

Expected limits, disconnect codes and the parcel grid come from the server crate's own constants
(`catalyst-comms`), so an expectation and the server it checks cannot drift apart.

## Running

```
comms-e2e doctor --stack-file <file>
comms-e2e list [--tag pulse]
comms-e2e run <scenario | --tag <tag> | --all> --stack-file <file>
              [--repeat <n>] [--seed <n>] [--strict] [--transport enet|webtransport]
              [--report <file.json>] [--report-hosts] [--work-dir <directory>]
              [--allow-host <host>] [--allow-hostile-host <host>]
              [--deadline <seconds>] [--timeline <n>] [--one-source-address]
comms-e2e console --stack-file <file> [--allow-host <host>] "/world_state" "/pulse_state"
comms-e2e address --key-env <VARIABLE>
```

- `doctor` dials every address the stack file names once (an HTTP address counts as there
  only if it answers 2xx), and checks the files and the
  environment variables it names (set or not set, never the value). Run it first: what the file
  names and is not there is `FAILED` and the exit code is 1; what the file leaves out is `absent`
  and only skips the scenarios that need it.
- The exit code is 0 only if nothing failed. `--strict` also fails the run on a skipped scenario.
- A failure prints the first expectation that did not hold, the seed that replays it
  (`--seed`), and the end of the timeline. `--report` writes every timeline in full.
- A scenario whose prerequisite the stack file lacks is **skipped** with the reason. It is never
  counted as passed.
- `--repeat` takes 1 to 100.
- Every scenario runs against loopback (a loopback address or the name `localhost`; any other
  name, one under `.localhost` included, is not loopback). A stack file that names any other
  host, a private or link-local address included, runs no
  scenario unless `--allow-host <host>` names each such host; a refused scenario fails before
  its first packet. Hostile scenarios (floods, malformed frames, bans, a restart of a process of
  the stack) need the host named with `--allow-hostile-host <host>` as well: `--allow-host`
  alone does not admit them. These flags prevent accidents and are not access control: a
  deployment is protected by the servers' own limits. `doctor` is not a scenario and dials any
  host.
- The report, printed or written with `--report`, names the loopback hosts of the stack file and
  counts the others; `--report-hosts` names them all.
- `console` starts one explorer behind the fault links and runs console commands in it: the way
  to see what a build answers before writing a scenario. It passes the same host guard as an
  ordinary scenario, so a stack that is not on loopback needs `--allow-host`.
- `run` and `console` keep their files (explorer logs) in a directory made new for each run,
  readable by the user alone where the platform has such modes: in the system's temporary
  directory, or in the `--work-dir` given.
- One repeat may take `--deadline` seconds (300 unless given). A failure shows the last
  `--timeline` entries (40 unless given).
- The explorer fronts are fixed ports, so one machine runs one explorer scenario or `console` at
  a time.

The tool does not start the stack. It dials one that is already up.

**Source addresses.** The servers count several budgets by source address: Pulse remembers 256
handshakes per address against replays, and admits 32 connections per address before
authentication. A run makes hundreds of handshakes from one machine. So against a loopback
stack, each scenario and repeat dials from an address of its own in `127.0.0.0/8`, which every
bot of that scenario shares; "many actors on one address" still means what it says inside a
scenario. `--one-source-address` turns this off. It is off by itself on macOS, where loopback is
one address, against a stack that is not on loopback, and for WebTransport without a pinned
certificate hash. The explorer always dials from the ordinary address.

**Transports.** `--transport webtransport` runs the Pulse scenarios again over WebTransport. The
expectations are the same on both: the same limits, the same disconnect codes. A server ends a
WebTransport session as it ends an ENet one: a frame over twice the message cap ends it with 16,
and a silent session stays open until the authentication timeout ends it with 2. Against a
server built before that change, `pulse-refusal-codes` and `pulse-auth-timeout` fail over
WebTransport. One thing a scenario does differently: QUIC ends a session nobody sends on for 30
seconds, so during a stall longer than that a quiet observer repeats its position
(`pulse-stall-and-resume`), which keeps the session open whether or not the server pings it. A
session the server ends without a Pulse disconnect code reaching the client fails the scenario
that expected the code, on either transport.

## The stack file

```toml
api = "http://127.0.0.1:5210"              # archipelago /ws, the gatekeeper, the statistics routes
pulse_enet = "127.0.0.1:5217"
pulse_metrics = "http://127.0.0.1:5217/metrics"
serve_metrics = "http://127.0.0.1:5211/metrics"
moderator_token_env = "CE2E_MODERATOR_TOKEN"       # the name of a variable, never the token
scene_listener_key_env = "CE2E_LISTENER_KEY"       # the wallet Pulse admits as a scene listener
restart_command = ["restart-the-api-process"]      # for stationary-credentials

[pulse_webtransport]
url = "https://127.0.0.1:5212/"
certificate_hash = "<hex SHA-256 of the server certificate>"

[livekit]
url = "http://127.0.0.1:5214"
key_env = "CE2E_LIVEKIT_KEY"
secret_env = "CE2E_LIVEKIT_SECRET"
client = "<path>/harness-util"             # the LiveKit client, see below
# without room-service keys, no [livekit] section: livekit_client = "<path>/harness-util" at
# the top of the file

[explorer]
binary = "<path>/headless"
arguments = ["--base-domain", "example.invalid"]
api_front = "127.0.0.1:6210"
pulse_front = "127.0.0.1:6217"
livekit_signal = { front = "127.0.0.1:6214", real = "127.0.0.1:5214" }

[limits]            # only what the servers run with other than their defaults,
                    # for example ban_sweep_interval_ms = 60000
[grid]
```

Only `pulse_enet` is required: with nothing else the Pulse scenarios run and the rest are
skipped. Secrets are read from the environment variables the file names; the file holds none.

**Fronts.** The explorer must dial the fault links, not the stack. So the stack is configured to
advertise the front addresses (the realm's archipelago adapter, the LiveKit URL in connection
strings), and the explorer is started with the fronts as its realm, gatekeeper and Pulse server.
The tool relays each front to the address behind it for as long as a scenario runs. Bots dial
the real addresses.

**The LiveKit client** is the explorer workspace's `harness-util join`
(`rendering/bevy-explorer/source/local-harness/presence/harness-util`), run as a child process.
The LiveKit SDK brings libwebrtc, which stays out of this workspace. The stack file names it with
`client` under `[livekit]` or, for a stack without room-service keys, with `livekit_client`;
`[livekit] client` wins when both are there.

**Tokens** go over TLS (`https`, `wss`) to any host and over a plain scheme only to loopback:
the moderator token, the LiveKit room-service token and a join connection string the tool mints
itself. Neither the gatekeeper client nor the room-service client follows a redirect.

## Scenarios

`comms-e2e list` prints them with what each guards. The groups:

| Group | Scenarios, each with its other tags | Need |
|---|---|---|
| `pulse` | `pulse-smoke` (`smoke`), `pulse-refusal-codes` (`refusals`), `pulse-auth-timeout` (`refusals`, `slow`), `pulse-rate-limits` (`limits`), `pulse-same-wallet-race` (`races`), `pulse-handshake-swarm` (`limits`, `swarm`), `pulse-churn` (`churn`), `pulse-realm-round-trip` (`realms`), `pulse-emote-lifecycle` (`emotes`), `pulse-resync-requests` (`resync`), `pulse-stall-and-resume` (`faults`, `slow`), `pulse-scene-listener` (`listeners`), `pulse-handshake-clock` (`refusals`, `probe`: fails unless a handshake signed at this machine's now gets in; measures how far the server's clock is behind or ahead, read across its 60 s window), `pulse-crowd` (`crowd`), `crowd-visit` (`crowd`, `visit`), `pulse-webtransport-bursts` (`webtransport`, `slow`) | Pulse. All but the last run on either transport (`--transport`). `crowd-visit` also needs a visit plan and the API (the realm description), see below. |
| `connections` | `connect-full-stack` (`connect`), `second-device-takeover` (`session`, `races`: Pulse ends the older device; the archipelago keeps one socket per session key and leaves the older socket open), `stale-socket-reconnect` (`session`, `faults`), `assignment-order` (`assignment`, `races`), `mints-under-churn` (`gatekeeper`, `races`), `ban-eviction` (`moderation`, `slow`), `stationary-credentials` (`restart`, `slow`), `livekit-same-identity` (`livekit`), `crowd-discovery` (`crowd`) | The API; LiveKit for the room joins, which are left out, with a note, without a LiveKit client |
| `explorer` | `explorer-uplink-stall` (`battle`, `faults`), `explorer-out-of-grid` (`battle`), `explorer-past-the-east-edge` (`battle`), `explorer-sleep-wake` (`battle`, `faults`), `explorer-port-unreachable` (`battle`, `faults`), `explorer-two-devices` (`races`), `explorer-resync` (`faults`). Probes: `explorer-replaced-by-another-device`, `explorer-replaces-another-device` (`probe`, `races`), `explorer-long-uplink-stall`, `explorer-reconnect-outside-grid` (`probe`, `faults`). The uplink over a slow path: `explorer-uplink-cadence` (`uplink`), `explorer-walk-800ms-round-trip`, `explorer-walk-1200ms-round-trip`, `explorer-walk-lossy-link`, `explorer-queued-stall-1200ms-round-trip`, `explorer-queued-stall-2000ms-round-trip` (`uplink`, `faults`) | The API and a headless explorer build |

**Crowds.** `pulse-crowd` and `crowd-discovery` put 20 players in one place at once. Their size
comes from `CE2E_CROWD_PLAYERS` (2 to 100), and `CE2E_CROWD_ROOM_PLAYERS` says how many of
`crowd-discovery`'s players join the island's LiveKit room (all unless it says fewer, `0` for
none). The two are read only when a crowd scenario is selected. With no LiveKit client in the
stack file, `crowd-discovery` leaves the room step out with a note while
`CE2E_CROWD_ROOM_PLAYERS` is unset, and fails when it asks for players in the room. In both
scenarios the 20 s each player has to see every other count from when the whole crowd has entered
Pulse. Every player dials from one address at the same moment, so a crowd larger than a correct
server admits from one address is refused before the first packet, naming the limit: Pulse's
`limits.max_concurrent_pre_auth_per_ip` for both, and the archipelago's
`limits.max_sockets_per_ip` for `crowd-discovery`. `crowd-discovery` checks an island's peer
list only when the connector sends one, and its report says whether it did
(`island_peers_sent`).

**Visits.** `crowd-visit` is the one scenario that goes where real clients are, so a person can
watch a crowd arrive in a client and the report shows which clients the crowd met. It runs only
when the run names it (`comms-e2e run crowd-visit`) and a place: `--all` and `--tag` leave it out
and the run says so as a skip, as it does without a place. Its variables are read only when the
run names it; its crowd is the crowd scenarios' (`CE2E_CROWD_PLAYERS`, at most 30 here):

| Variable | What it says |
|---|---|
| `CE2E_VISIT_REALM` | The realm name exactly as the clients there send it to Pulse: a realm's `/about` `configurations.realmName` |
| `CE2E_VISIT_PARCEL` | The parcel, `x,y`, the crowd stands around (on a grid 2 m apart, at most 30 players) |
| `CE2E_VISIT_SECONDS` | How long it stays, walking back and forth, before it leaves (120 unless given, 1 to 900); give `--deadline` above it |
| `CE2E_VISIT_EXPECT` | Wallets, comma-separated, every player must catch a Pulse join event of |
| `CE2E_VISIT_LABEL` | The crowd's label (`ce2e-visit` unless given; 1 to 32 lowercase letters, digits and dashes): its wallets come from it |

The report notes every join event each player caught (`visitor_joins_by_player` for wallets
outside the crowd, `crowd_join_events_caught_by_player` for the crowd itself), for each expected
wallet how many players caught it (`expected_caught_by`), the crowd's wallets by player
(`crowd_wallets`) and samples of what each player held while it stayed (`held_samples`).

**Profiles.** A client draws another player only once it has that player's profile, which it
reads from a catalyst's lambdas. So before the crowd enters, each player makes sure the realm
serves a profile for its wallet: it reads the realm description (`<api>/about`) for the content
server (`content.publicUrl`) and the lambdas (`lambdas.publicUrl`), asks the lambdas for the
wallet's profile, and deploys one to `POST <content.publicUrl>/entities` unless they serve
exactly the profile it would deploy: the same name and every other field it sets, and no field it
does not set (the lambdas' own `userId`, `ethAddress` and snapshots aside). A profile deployed over
another one is stamped after it, since the content server keeps the newer of the two. The
deployment is the one the explorer makes for its own profile: an entity
file with no files attached, named by its CIDv1 and signed by the wallet. The profile is named
`ce2e-visit-NN` (`NN` the player's index), claims no name, wears base avatars only, its body shape
and colours varied by index, and has no snapshots. Each player then announces that profile's
version: in its handshake and in a profile announcement before it enters the realm, so every join
event Pulse sends of it carries the version, which the scenario checks. Other scenarios announce
no profile.

- A player's wallet comes from the crowd label and its index, not from the run: every visit under
  one label is the same crowd, and its profiles are deployed once however many visits are made.
  Two visits at the same time need different labels: under one label they are the same wallets
  and sessions twice, and each ends the other's players.
- Deploying writes to a server. Only `crowd-visit` does it, after the host guard has admitted the
  run, and the content server and the lambdas the realm description names must pass the same
  guard: loopback, or a host `--allow-host` names. A refused deployment fails the visit before
  anyone enters; a 429 or 503 that says when to come back (`Retry-After`, up to 60 s) is sent once
  more. The report notes `profiles_deployed` (of them `profiles_replaced`, deployed over another
  profile served for the wallet), `profiles_already_served`,
  `profiles_sent_again_after_retry_after` and, after a deployment, how many the lambdas served
  before the crowd entered (`profiles_served_before_entering`, waited for up to 30 s).
- What a visit leaves behind on a deployment: one profile entity per wallet of each label used,
  `ce2e-visit-00` to `ce2e-visit-NN`, in the content server's history and active entities, served
  by its lambdas to anyone who asks. A content server deletes no entity; a later deployment for the
  same wallet replaces the active profile, and only a deployment's own operators can remove it.
  The wallets' keys derive from the label and are not secret: anyone can deploy for them, and the
  next visit deploys its own profile over whatever else it finds served.

Races repeat by default and take their order and gaps from the seed, so a failure replays.

The `uplink` scenarios walk the explorer back and forth and read its movement from the Pulse
link's history, not from the explorer: datagrams of 20 bytes or more are messages (movement while
it walks), smaller ones the transport's own. A gap longer than one and a half beats of the
measured walk (and 180 ms) is a silence, counted between two of them and from the start of a walk
leg to its first and from its last to the leg's end, so movement that stops shows. A regular
slower beat is no silence: on a long round trip the explorer spaces its movement out on purpose,
at most 8 inputs in a round trip and 600 ms. So a walk must also keep half the rate that spacing
allows (or half the plain-link rate, where that is lower): 2.9 a second at 800 ms, 2.2 at 1.2 s,
4 on the lossy path, where one movement a round trip is 1.25, 0.83 and about 3.3. A queued stall
walks on when the observer sees new movement after the released burst has landed, and the uplink
after the landing keeps that rate. Each scenario walks on the plain link first, as its own
control, and samples `/pulse_state` while it walks, so a report puts each silence beside what the
explorer said its uplink gate did. The measure cannot tell movement from another message of the
same size, nor a silence a long frame made from one the gate made: the readout says which.
`explorer-uplink-cadence` checks the measure itself, walking and then standing still once the last
leg has ended.

### What the explorer answers

The headless explorer's console is the scenarios' view into the client. These are the commands
they use and the shape of each answer:

| Command | Answer |
|---|---|
| `/world_state` | JSON: `inWorld`, `address`, `realmName`, `position` (`[x, y, z]`), `outOfWorld`, `arrivals` |
| `/pulse_state` | JSON: `state` (`authenticated`, `connecting`, `waiting`, `terminal disconnect`, …), `resyncs`, traffic per second, and `uplink`: `stalled`, `held` (teleport, emote, input), `unacknowledgedReliable`, `probes` |
| `/connectivity_state` | JSON: `rooms`, each with a `scope` (`realm`, `private_chat`) and a `state` (`connected`, `connecting`, `identity_conflict`, …) |
| `/transport_state` | JSON: `total` and `transports`, each with a `type` (`Pulse`, `Archipelago`) and its `players` |
| `/move_player_to x y z [seconds]` | Text. Moves the player at once, which Pulse sees as a teleport, or over the seconds given, which it sees as movement. Preview mode only, so the scenarios that move the player start the explorer with `--preview`. |
| `/connected_players`, `/player_position`, `/get_user_data` | Text |

A player in the headless explorer cannot play an emote from the console, so the emote limits are
tested at the wire by `pulse-refusal-codes`, not through the explorer.

## Writing a scenario

A scenario is an async function and a `static Scenario` naming it, its tags, what it needs and
the problems it guards. Three rules keep scenarios worth having:

- **Wait on what was observed, with a deadline.** `expect_or`, `expect_async`, `holds_for`.
  No sleeps, except the length of a fault a scenario states. A probe that answers
  `Err("fatal: …")` ends the wait at once: use `bot.alive()?` so a dead connection fails the
  scenario with its code, not at a deadline.
- **Assert on views, not on what a bot assumes.** `PulseView`, `ArchipelagoView`,
  `LiveKitView` hold what the servers said.
- **Say what broke.** The expectation text is the first line of a failure.

The crate's own tests need no stack: they start a Pulse server in the test process
(`tests/support`) and run the Pulse scenarios, the bots and the fault link against it.

## Limits

- Voice calls, load and soak runs, and a browser client are outside what this tool does.
- On Windows, give console commands that start with `/` to the tool from a shell that does not
  rewrite them as paths (in Git Bash: `MSYS_NO_PATHCONV=1`).
