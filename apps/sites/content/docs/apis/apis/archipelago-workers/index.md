# Comms statistics and WebSocket API

In dcl-one, Catalyst serves the statistics API and the signed WebSocket connector. Pulse owns proximity clustering and the statistics population. Gatekeeper authorizes room assignments and issues LiveKit credentials.

## Statistics

Use the `archipelago-ea-stats` service subdomain or the `/archipelago-ea-stats` mount on the Catalyst API. Paths below are relative to that mount.

| Method and path | Returns |
| --- | --- |
| `GET /parcels` | Peer counts by parcel |
| `GET /peers` and `GET /peers/:id` | Observed positions for peers or one peer |
| `GET /islands` and `GET /islands/:id` | Island summaries or one island |
| `GET /hot-scenes` | Scene activity with content metadata |
| `GET /core-status` | Clustering discovery status |

The `/comms` aliases also serve the parcel, peer, and island reads. Only observations from authenticated WebSocket sessions contribute peers to this view.

## WebSocket connector

Open the endpoint advertised by the realm. The primary Catalyst server accepts WebSocket upgrades at `/ws` and at `/ws` under the `/archipelago-ea-ws-connector` and `/archipelago-ea-stats` mounts. See the [Realm WebSocket connector](/docs/contributor/communications/archipelago/) for the handshake and assignment messages.
