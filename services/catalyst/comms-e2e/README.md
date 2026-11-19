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
| Fault link (`fault`) | A relay a client dials instead of the stack. A scenario can stall, drop, delay, reorder or duplicate either direction, half-open or cut the flows that exist, orphan TCP connections (closed towards the client, left open towards the stack) and close a UDP port so it answers "unreachable". |
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
              [--report <file.json>] [--work-dir <directory>] [--allow-host <host>]
              [--deadline <seconds>] [--timeline <n>] [--one-source-address]
comms-e2e console --stack-file <file> "/world_state" "/pulse_state"
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
- Hostile scenarios (floods, malformed frames, bans, a restart of a process of the stack) refuse
  a target that is not loopback (`localhost` and `*.localhost` count as loopback) unless
  `--allow-host` names it. A private or link-local address has to be named too.
- `console` starts one explorer behind the fault links and runs console commands in it: the way
  to see what a build answers before writing a scenario.
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
The LiveKit SDK brings libwebrtc, which stays out of this workspace.

## Scenarios

`comms-e2e list` prints them with what each guards. The groups:

| Tag | Scenarios | Need |
|---|---|---|
| `pulse` | Smoke, refusal codes, the authentication timeout, rate limits, the same-wallet race, the handshake swarm, churn, a realm round trip, the emote lifecycle, resync requests, stall and resume, the scene listener, bursts over WebTransport | Pulse. All but the last run on either transport (`--transport`). |
| `connections` | The full stack for one wallet, a second device (Pulse ends the older one; the archipelago keeps one socket per session key and leaves the older socket open), a stale socket, assignment order, mints under churn, ban eviction, credentials across a restart, two LiveKit joins with one identity | The API, and LiveKit for the joins |
| `explorer` | An uplink stall while teleporting, off the north edge of the grid, off the east edge, a stale archipelago socket, an unreachable Pulse port, two explorers with one wallet and one signing key, packets from Pulse out of order. Probes (`probe`): another device of the explorer's wallet signing in after it or before it, an uplink stall longer than the transport waits, a reconnect outside the grid | The API and a headless explorer build |

Races repeat by default and take their order and gaps from the seed, so a failure replays.

### What the explorer answers

The headless explorer's console is the scenarios' view into the client. These are the commands
they use and the shape of each answer:

| Command | Answer |
|---|---|
| `/world_state` | JSON: `inWorld`, `address`, `realmName`, `position` (`[x, y, z]`), `outOfWorld`, `arrivals` |
| `/pulse_state` | JSON: `state` (`authenticated`, `connecting`, `waiting`, `terminal disconnect`, …), `resyncs`, traffic per second |
| `/connectivity_state` | JSON: `rooms`, each with a `scope` (`realm`, `private_chat`) and a `state` (`connected`, `connecting`, `identity_conflict`, …) |
| `/transport_state` | JSON: `total` and `transports`, each with a `type` (`Pulse`, `Archipelago`) and its `players` |
| `/move_player_to x y z` | Text. Moves the player at once, which Pulse sees as a teleport. Preview mode only, so the scenarios that move the player start the explorer with `--preview`. |
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
