const apiUrl = "/docs/apis/apis/archipelago-workers/";
const connectorUrl = "/docs/contributor/communications/archipelago/";
const apiTitle = "Comms statistics and WebSocket API";
const connectorTitle = "Realm WebSocket connector";
const apiLink = `[${apiTitle}](${apiUrl})`;
const connectorLink = `[${connectorTitle}](${connectorUrl})`;

const titles = new Map([
  [apiUrl, apiTitle],
  ["/docs/apis/archipelago-workers/", apiTitle],
  [connectorUrl, connectorTitle],
]);

export function commsDocTitle(url: string | null): string | undefined {
  return url === null ? undefined : titles.get(url);
}

const statisticsApi = [
  `# ${apiTitle}`,
  "In dcl-one, Catalyst serves the statistics API and the signed WebSocket connector. Pulse owns proximity clustering and the statistics population. Gatekeeper authorizes room assignments and issues LiveKit credentials.",
  "## Statistics",
  "Use the `archipelago-ea-stats` service subdomain or the `/archipelago-ea-stats` mount on the Catalyst API. Paths below are relative to that mount.",
  [
    "| Method and path | Returns |",
    "| --- | --- |",
    "| `GET /parcels` | Peer counts by parcel |",
    "| `GET /peers` and `GET /peers/:id` | Observed positions for peers or one peer |",
    "| `GET /islands` and `GET /islands/:id` | Island summaries or one island |",
    "| `GET /hot-scenes` | Scene activity with content metadata |",
    "| `GET /core-status` | Clustering discovery status |",
  ].join("\n"),
  "The `/comms` aliases also serve the parcel, peer, and island reads. The shared Catalyst API root exposes those reads under `/stats` as well. For example, use `/stats/peers` at the API root. Only observations from authenticated WebSocket sessions contribute peers to this view.",
  "## WebSocket connector",
  `Open the endpoint advertised by the realm. The primary Catalyst server accepts WebSocket upgrades at \`/ws\` and at \`/ws\` under the \`/archipelago-ea-ws-connector\` and \`/archipelago-ea-stats\` mounts. See the ${connectorLink} for the handshake and assignment messages.`,
].join("\n\n");

const catalystOwner: readonly (readonly [RegExp, string])[] = [
  [/^Ever felt curious/m, "This guide covers the separate `catalyst-owner` distribution. For dcl-one, follow the [local development guide](https://github.com/decentraland/dcl-one/blob/main/docs/development.md).\n\nEver felt curious"],
  [/^(> \.\/init\.sh)\n[\s\S]*?(?=^```)/m, "$1\n"],
];

const updates: Record<string, readonly (readonly [string | RegExp, string])[]> = {
  "apis/README.md": [
    [/^\| Archipelago Workers.*$/m, `| ${apiLink} | Room signaling and statistics | Catalyst WebSocket sessions, peer and island reads, and hot scenes |`],
  ],
  "apis/apis/archipelago-workers/README.md": [[/^# Archipelago Workers\s*$/, statisticsApi]],
  "apis/archipelago-workers/README.md": [[/^# Archipelago Workers\s*$/, `# ${apiTitle}\n\nSee the ${apiLink} reference for endpoints and client behavior.`]],
  "contributor/architecture/catalyst.md": [
    [/^## Archipelago Workers \(Comms\)\n[\s\S]*?(?=^\*\*\*$)/m,
      `<span id="archipelago-workers-comms"></span>\n\n## Catalyst communications\n\nThe primary \`catalyst serve\` process runs the signed ${connectorLink} and Gatekeeper. Gatekeeper enforces room policy and issues LiveKit credentials. The separate \`catalyst pulse\` process synchronizes avatars, clusters nearby players, and owns the statistics population. Every \`catalyst serve\` process exposes the read-only statistics API.\n\nSee the ${apiLink} reference.\n\n`],
  ],
  "contributor/architecture/infrastructure.md": [
    ["* Archipelago Workers - Default voice", "* Catalyst Gatekeeper - LiveKit access for island and other voice rooms"],
    [/^\| Archipelago\s+\| Position updates \|$/m, "| Pulse and Catalyst | Cluster updates, room lookup, and statistics snapshots |"],
  ],
  "contributor/architecture/services.md": [
    ["Decentraland's backend is composed of distributed microservices organized into logical layers:", "The backend exposes the following groups of APIs. In dcl-one, Catalyst serves multiple API areas from a shared process."],
    ["* Archipelago Workers - Provides island/cluster data", "* Pulse - Provides cluster updates and statistics snapshots"],
    ["* Aggregates Archipelago statistics", "* Reads Catalyst's communications statistics"],
    [/^### Archipelago Workers\n[\s\S]*?(?=^\*\*\*$)/m,
      `<span id="archipelago-workers"></span>\n\n### Pulse and Catalyst communications\n\nPulse runs as \`catalyst pulse\` and synchronizes avatars over ENet or WebTransport. It clusters nearby players and sends cluster changes and statistics to Catalyst over NATS.\n\nThe primary \`catalyst serve\` runs the signed ${connectorLink}. Gatekeeper checks room access and sends LiveKit assignments through that connector. Each \`catalyst serve\` exposes read-only statistics.\n\nSee the ${apiLink} reference.\n\n`],
    ["* Archipelago Stats - Real-time user counts per scene", "* Catalyst communications statistics - Scene activity and peer counts"],
    ["* Archipelago - Queries online status and locations", "* Catalyst communications - Supplies Pulse topology for friends' online status and can supply peer reads for the community online filter"],
    ["* **Archipelago** - Routes position updates between islands", "* **Pulse and Catalyst** - Exchange cluster updates, room lookups, and statistics over NATS"],
    ["| Realm Provider        | Catalyst, Archipelago Stats    | All clients                    |", "| Realm Provider        | Catalyst APIs                  | All clients                    |"],
    ["| Archipelago Workers   | NATS, LiveKit                  | Realm Provider, Places         |", "| Pulse                 | NATS, Catalyst room authority  | Explorer, Catalyst             |"],
    ["| Social Service        | Catalyst, NATS, Archipelago    | Clients                        |", "| Social Service        | Catalyst, NATS                 | Clients                        |"],
    ["| Places                | Catalyst, Archipelago, Comms   | Clients                        |", "| Places                | Catalyst content and comms     | Clients                        |"],
  ],
  "contributor/catalyst/glossary.md": [
    [/^Main responsibilities were to determine which are the candidates.*$/m, "It selected peers for direct connections and handled WebRTC signaling."],
    [/^### Archipelago Service\n\n[^\n]+/m, `<span id="archipelago-service"></span>\n\n### Realm WebSocket connector\n\nThe signed ${connectorLink} in the primary Catalyst API delivers Gatekeeper room assignments. The \`archipelago:\` adapter prefix identifies its compatibility protocol. Pulse groups nearby players into islands.`],
  ],
  "contributor/communications/README.md": [[`[archipelago](${connectorUrl})`, connectorLink]],
  "contributor/communications/main-realm.md": [
    [/^Decentraland has as many realms[\s\S]*$/, [
      "MAIN is the shared realm players can join while fetching content from different Catalyst nodes.",
      "Read the selected realm’s `/about` response to discover its content and communications endpoints. The [realm description contract](https://adr.decentraland.org/adr/ADR-110) describes how those endpoints are advertised.",
      `In dcl-one, Pulse handles player movement and clustering. The primary Catalyst API serves the signed ${connectorLink}, which delivers Gatekeeper’s assignments for [LiveKit rooms](/docs/contributor/communications/transport-types/livekit/). The ${apiLink} reference lists the compatible statistics routes.`,
    ].join("\n\n")],
  ],
  "contributor/communications/overview.md": [
    [/^Since most of this functionality[\s\S]*?(?=^## Connections)/m, [
      "Pulse synchronizes avatar movement and groups nearby players into temporary clusters called islands. Gatekeeper controls room access and issues LiveKit credentials. The signed WebSocket connector delivers those assignments to clients.",
      '<span id="lifecycle"></span>\n\n## Client Life-cycle',
      `1. Read the realm’s \`/about\` response to discover its communications adapter.\n2. If the realm advertises the signed ${connectorLink}, connect and authenticate. Explorer also connects to Pulse for avatar synchronization.\n3. Use the transport URI in the assignment to join the island’s media room.\n4. Process later assignments as the selected room changes. Close the connections when the session ends.`,
      "",
    ].join("\n\n")],
    ["Comms connections are mainly websocket-based, although some transports may use other strategies.", "The realm connector uses WebSocket. Pulse uses ENet or WebTransport. LiveKit transports media over WebRTC."],
    [/^Islands are highly dynamic[\s\S]*?(?=^## Messages)/m, `An island is a temporary proximity group. Pulse updates membership as players move. Gatekeeper maps assignments to rooms and the connector supplies the URI needed to join. See the ${connectorLink} for the assignment flow.\n\n`],
    [`[Archipelago](${connectorUrl})`, connectorLink],
  ],
  "contributor/communications/archipelago.md": [
    [/^Archipelago is the realm service[\s\S]*?(?=^## Connecting)/m, "The primary Catalyst API serves a signed WebSocket connector for realm signaling. Pulse clusters players, and Gatekeeper authorizes assignments and issues the room credentials sent over the connector. The adapter prefix `archipelago:` identifies the compatible protobuf protocol.\n\n"],
    [/^To begin, clients must open[\s\S]*?(?=^## Authenticating)/m, "Read `comms.adapter` in the realm’s `/about` response and use the advertised WebSocket URL. For example, `archipelago:archipelago:wss://realm.example/ws` advertises `wss://realm.example/ws`. By default, clients have 10 seconds after opening the socket to send `ChallengeRequestMessage`, then 60 seconds after receiving the challenge to send `SignedChallengeMessage`.\n\nA realm may instead advertise a fixed transport through `comms.fixedAdapter`. In that case, use that transport directly.\n\n"],
    ["After opening a connection to Archipelago, clients must begin by requesting and signing a challenge from the service to verify their identity.", "After opening the socket, request and sign a challenge to verify your identity."],
    [/^```mermaid\nsequenceDiagram\n    participant Client\n    participant Archipelago\n[\s\S]*?^```/m, [
      "```mermaid",
      "sequenceDiagram",
      "    participant Client",
      "    participant Connector",
      "    Client->>Connector: ChallengeRequestMessage",
      "    Connector->>Client: ChallengeResponseMessage",
      "    Client->>Connector: SignedChallengeMessage",
      "    Connector->>Client: WelcomeMessage",
      "```",
    ].join("\n")],
    [/^During their session, clients must periodically send[\s\S]*?(?=^## Getting Island Assignments)/m, "The connector accepts [`Heartbeat`](#heartbeat-message) position reports from an authenticated socket. These reports feed the compatibility statistics view and can provide position or room intent to Pulse. Explorer also sends movement updates over its separate Pulse connection. WebSocket ping and pong frames maintain the transport connection.\n\n"],
    ["Shortly after the first heartbeat, Archipelago will send the client their first [`IslandChangedMessage`][IslandChangedMessage].", "After authentication, the connector announces the session to Gatekeeper. It sends an [`IslandChangedMessage`][IslandChangedMessage] when a room assignment is available."],
    [/^During the session, Archipelago may send[\s\S]*?(?=^Clients must listen)/m, "The connector can send a new assignment when the selected room changes or refreshed credentials are available.\n\n"],
    ["their Ethereum address (i.e. their public key)", "their Ethereum address"],
    ["Sent by the client at regular intervals (typically once per second), to update Archipelago on their position and/or request an island assignment.", "Clients can send this message periodically to report their position to the connector."],
    ["The first `Heartbeat` message a client sends is quickly followed by an [`IslandChangedMessage`](#islandchangedmessage) from Archipelago. Subsequent updates, however, are independent of island assignments. Clients should not expect a `Heartbeat` to be responded.", "Assignments arrive asynchronously. An individual `Heartbeat` has no corresponding acknowledgement."],
    ["| The ID of an island the client would like to be assigned to |", "| Realm hint for the position update |"],
    [/^When the `desired_room` parameter is included.*$/m, "Catalyst interprets `desired_room` as a realm hint. Gatekeeper still applies room access policy."],
    ["Description.\n\n| Field", "| Field"],
    [/\| `peers`[^\n]+/, "| `peers`          | `map<string, Position>` | Peer positions, when the assignment includes them |"],
    [/^Clients that receive an `IslandChangedMessage` should end.*$/m, "Use `conn_str` to join the assigned room. Keep a healthy connection when an update refers to the same room."],
    [/^The `peers` field contains.*$/m, "The protobuf schema defines the `peers` field. Use the appropriate room or Pulse transport for ongoing player updates."],
    ["Sent by Archipelago in response to a", "The connector sends this in response to a"],
    ["Sent by Archipelago after successful authentication.", "The connector sends this after successful authentication."],
    ["Sent by Archipelago when the client is (re)assigned to an island.", "The connector sends this when it has an assignment for the client."],
    ["Sent by Archipelago before closing a connection.", "The connector can send this before closing the session."],
    ["Archipelago's reason for closing the connection", "The reason the connector closed the connection"],
    [/^Sent by Archipelago when a peer is (assigned to|removed from) the client's island\.$/gm, "This message is defined by the compatibility schema. Catalyst sends room assignments through `IslandChangedMessage`."],
    [/^(## (?:Connecting|Authenticating)) \{#[a-z]+\}$/gm, "$1"],
    [/^(#{2,3} .+?) \{#([\w-]+)\}$/gm, '<span id="$2"></span>\n\n$1'],
  ],
  "contributor/communications/transport-types/livekit.md": [
    [/^The \[LiveKit\].*$/m, `Use the [LiveKit](https://livekit.io/) transport when the ${connectorLink} supplies a LiveKit connection string or when the realm advertises LiveKit as its fixed transport.`],
    [`generated by [Archipelago](${connectorUrl}) when the client authenticates`, "issued by Gatekeeper for a room the client may join"],
    ["provided by Archipelago is already verified and authorized", "carries authorization to join the room"],
  ],
  "contributor/communications/transport-types/websocket.md": [
    [/^The websocket transport is one.*$/m, "The `ws-room:` WebSocket transport relays messages in a room. Use it when the realm supplies a `ws-room:` connection string."],
    [`from [Archipelago](${connectorUrl})`, `from the ${connectorLink}`],
  ],
  "contributor/communications/transport-types/signed-login.md": [
    [/^The Signed Login transport.*$/m, "The Signed Login transport lets a server choose a room for a client through an HTTP endpoint."],
  ],
  "contributor/tutorials/getting-started.md": catalystOwner,
  "contributor/tutorials/how-to-run-a-catalyst.md": catalystOwner,
  "player/decentraland-in-world/overview.md": [["and it's archipelago of off-map Worlds", "and its off-map Worlds"]],
};

export function updateCommsDocs(source: string, markdown: string): string {
  for (const [before, after] of updates[source] ?? []) {
    const revised = typeof before === "string" ? markdown.replaceAll(before, after) : markdown.replace(before, after);
    if (revised === markdown) {
      throw new Error(`[docs-content] comms prose rule did not match in ${source}: ${before}`);
    }
    markdown = revised;
  }
  return markdown;
}
