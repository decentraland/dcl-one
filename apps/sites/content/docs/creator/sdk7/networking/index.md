> Build multiplayer and networked experiences

# Networking

Create multiplayer experiences by implementing networking and synchronization between players. This section covers serverless peer-to-peer sync, the native Multiplayer Server, and direct API/WebSocket connections.

## Approaches

| Approach | Best for | Complexity |
|---|---|---|
| [Serverless Multiplayer](/docs/creator/sdk7/networking/serverless-multiplayer/) | Simple shared scenes, no incentive to cheat | Low |
| [Multiplayer Server](/docs/creator/sdk7/networking/authoritative-servers/) | Competitive games, anti-cheat, server-validated state | Medium |
| [Network Connections](/docs/creator/sdk7/networking/network-connections/) | Fetching external data, REST APIs, WebSockets | Low–Medium |
| [Third Party Servers](/docs/creator/sdk7/networking/third-party-servers/) | REST APIs, WebSockets, Colyseus, hybrid approaches | Medium |

## Multiplayer Server

The native SDK Multiplayer Server lets you write one TypeScript codebase that runs on both client and a headless server process. The server validates all state changes and is the single source of truth for all players.

Use this when players have an incentive to cheat: the server reads verified player positions and blocks any client from modifying server-owned state.

→ [Multiplayer Server](/docs/creator/sdk7/networking/authoritative-servers/)

## Serverless Multiplayer

The simplest option: mark entities as synced or send MessageBus messages between players directly, with no server required. State is not persisted when all players leave.

→ [Serverless Multiplayer](/docs/creator/sdk7/networking/serverless-multiplayer/)

## Network Connections

Connect your scene to external REST APIs or WebSocket servers to fetch live data, post scores, or integrate with third-party services.

→ [Network Connections](/docs/creator/sdk7/networking/network-connections/)
