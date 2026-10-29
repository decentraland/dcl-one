> Build multiplayer Decentraland scenes with a third party server.

# Third party Servers

The recommended approach for multiplayer interactions in your scene is using the [Multiplayer Server](/docs/creator/sdk7/networking/authoritative-servers/). This document covers other options that are also supported.

- **REST API + DB**: Good for data that changes infrequently (leaderboards, guestbooks). Players poll the API for updates; state persists between sessions. See [Network Connections](/docs/creator/sdk7/networking/network-connections/) for how to make `fetch` requests from a scene.
- **WebSocket server**: Allows real-time two-way communication. See [Network Connections](/docs/creator/sdk7/networking/network-connections/) for WebSocket usage. Libraries like [Colyseus](https://colyseus.io/) work well with the Decentraland SDK.


> [!WARNING]
> **📔 Note**: Third-party servers don't integrate with `syncEntity`, `validateBeforeChange`, or `Storage` — you'll need to implement your own state management and sync logic.


It's also possible to opt for a hybrid approach where changes are notified between players via [serverless-multiplayer](/docs/creator/sdk7/networking/serverless-multiplayer/), but the final state is also stored via a REST API for future visitors.

Here are some examples of scenes that use a third-party server:

- [Leaderboard](https://github.com/decentraland/sdk7-goerli-plaza/tree/main/leader-board)
- [Guestbook](https://github.com/decentraland/sdk7-goerli-plaza/tree/main/guest-book-api)
- [Validate authenticity](https://github.com/decentraland/sdk7-goerli-plaza/tree/main/validate-player-authenticity)

## Testing Locally

To preview a scene that uses a 3rd party server, you must run both the scene and the server it relies on. The server can be run locally in the same machine as the preview, as an easier way to test it. When running locally, the server can use unsafe `http` or `ws` connections, for easier setup.

To start the server, go to the `/server` folder and run `npm run start`.

Open the preview in two separate windows, each window is treated as a separate player. Connect each window with a different address. Both clients will connect to the same local server instance.

Using the Creator Hub, click the Preview button a second time, and that opens a second Decentraland explorer window. You must connect on both windows with different addresses. The same sessions will remain open as the scene reloads.

As an alternative, you can open a second Decentraland explorer window by writing the following into a browser URL:

> `decentraland://realm=http://127.0.0.1:8000&local-scene=true&debug=true&multi-instance=true`

## Separate realms

Players in decentraland exist in many separate _realms_. Players in different realms cant see each other, interact or chat with each other, even if they're standing on the same parcels. Dividing players like this allows Decentraland to handle an unlimited amount of players without running into any limitations. It also pairs players that are in close regions, to ensure that ping times between players that interact are acceptable.

If your scene sends data to a 3rd party server to sync changes between players in real time, then it's important that changes are only synced between players that are on the same realm. You should handle all changes that belong to one realm as separate from those on a different realm. Otherwise, players will see things change in a spooky way, without anyone making the change.

See how to obtain the realm for each player in [get player data](/docs/creator/sdk7/interactivity/user-data/)

## Multiplayer persistance

Unlike local scenes that are newly mounted each time a player walks into them, scenes that use 3rd party servers have a life span that extends well beyond when the player enters and leaves the scene.

You must therefore design the experience taking into account that player won't always find the scene in the same initial state. Any changes made to the scene will linger on for other players to find, you must make sure that these don't interfere with future player's experiences in an undesired way.

### Reset the state

When loading the scene, make sure its built based on the shared information stored in the server, and not in a default state.

In some cases, it makes sense to include some kind of reset button in the scene. Pressing the reset button would reset the scene gracefully.

Sometimes, this just implies setting the variables in the scene state back to default values. But resetting the scene might also involve unsubscribing listeners and stopping loops in the server side. If empty loops remain each time the scene is reset, these would keep piling up and will have an ill effect on the scene's performance.
