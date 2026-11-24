# main-realm

MAIN is the shared realm players can join while fetching content from different Catalyst nodes.

Read the selected realm’s `/about` response to discover its content and communications endpoints. The [realm description contract](https://adr.decentraland.org/adr/ADR-110) describes how those endpoints are advertised.

In dcl-one, Pulse handles player movement and clustering. The primary Catalyst API serves the signed [Realm WebSocket connector](/docs/contributor/communications/archipelago/), which delivers Gatekeeper’s assignments for [LiveKit rooms](/docs/contributor/communications/transport-types/livekit/). The [Comms statistics and WebSocket API](/docs/apis/apis/archipelago-workers/) reference lists the compatible statistics routes.
