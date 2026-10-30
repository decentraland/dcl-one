> Obtain data from the context where your scene is running and the scene itself.

# Runtime Data

## Get Decentraland Time

Decentraland follows a day/night cycle that takes 2 hours to be completed, so there are 12 full cycles every day. Players can also change the settings to experience a specific fixed time of day, for example to always see Decentraland with a 10pm night sky. For this reason, Decentraland time may vary from one player to another.

Use `getWorldTime()` to fetch the time of day that the player is experiencing inside Decentraland.

```ts
import { getWorldTime } from '~system/Runtime'

executeTask(async () => {
  let time = await getWorldTime({})
  console.log(time.seconds)
})
```


> [!NOTE]
> **💡 Tip**: The `getWorldTime()` function is asynchronous. See [Asynchronous functions](/docs/creator/sdk7/programming-patterns/async-functions/) if you're not familiar with those.


`getWorldTime()` returns an object with a `seconds` property. This property indicates how many seconds have passed (in Decentraland time) since the start of the day, assuming the full cycle lasts 24 hours. Divide the seconds value by 60 to obtain minutes, and by 60 again to obtain the hours since the start of the day. For example, if the `seconds` value is _36000_, it corresponds to _10 AM_.

In Decentraland time, the sun always rises at 6:15 and sets at 19:50.

You could use this information to change the scene accordingly, for example to play bird sounds when there's daylight and crickets when it's dark, or to turn the emissive materials on street lamps when it's dark.

```ts
import { getWorldTime } from '~system/Runtime'

executeTask(async () => {
  let time = await getWorldTime({})
  console.log(time.seconds)
  if (time.seconds < 6.25 * 60 * 60 || time.seconds > 19.85 * 60 * 60) {
    // night time
    console.log('playing cricket sounds')
  } else {
    // day time
    console.log('playing bird sounds')
  }
})
```

## Get realm data

Players in decentraland exist in several separate _realms_. Players in different realms can't see each other, interact or chat with each other, even if they're standing on the same parcels. Dividing players like this allows Decentraland to handle an unlimited amount of players without running into any limitations. It also pairs players who are in close regions, to ensure that ping times between players that interact are acceptable.

If your scene sends data to a [3rd party server](/docs/creator/sdk7/networking/third-party-servers/) to sync changes between players in real time, then it's often important that changes are only synced between players that are on the same realm. You should handle all changes that belong to one realm as separate from those on a different realm. Otherwise, players will see things change in a spooky way, without anyone making the change.

```ts
import { getRealm } from '~system/Runtime'

executeTask(async () => {
  const { realmInfo } = await getRealm({})
  if (!realmInfo) return
  console.log(`You are in the realm: `, realmInfo.realmName)
})
```


> [!NOTE]
> **💡 Tip**: The `getRealm()` function is asynchronous. See [Asynchronous functions](/docs/creator/sdk7/programming-patterns/async-functions/) if you're not familiar with those.


Decentraland handles its communications between players (including player positions, chat, messageBus messages and smart item state changes) through a decentralized network of communication servers, each of these servers is called a **Realm**. Each one of these servers can support multiple separate **rooms** (also called **islands**), each grouping a different set of players that are near each other on the Decentraland map.

The `getRealm()` function returns the following information:

* `baseUrl`: _(string)_ The domain of the realm server
* `realmName`: _(string)_ The name of the realm server
* `networkId`: _(number)_ The Ethereum network
* `commsAdapter`: _(string)_ Comms adapter, removing all query parameters (credentials)
* `isPreview`: _(boolean)_ True if the scene is running as a local preview, instead of published in Decentraland.
* `isConnectedSceneRoom`: _(boolean)_ True if the user is connected to the scene room.


> [!WARNING]
> **📔 Note**: The `layer` property is deprecated, and should be avoided.


As players move through the map, they may switch rooms to be grouped with those players who are now closest to them. Rooms also shift their borders dynamically to fit a manageable group of people, so even if a player stands still, as players enter and leave the world, the player could find themselves on another room. Players in a same `room` are communicated, and will share messages across the MessageBus even if they;re too far to see each other. Players in a same server but in different rooms are not currently communicating, but they might get communicated as they move around the map and change rooms.

To react to changes regarding the player's realm or room, use the `onChange` function on the `RealmInfo` component, which the engine adds to the `engine.RootEntity`. This component holds the same fields returned by `getRealm()`.

```ts
import { engine, RealmInfo } from '@dcl/sdk/ecs'

export function main() {
	RealmInfo.onChange(engine.RootEntity, (realmInfo) => {
		if (!realmInfo) return
		console.log('Realm changed: ', realmInfo.realmName)
	})
}
```


> [!WARNING]
> **📔 Note**: When the scene first loads, there might not yet be a room assigned for the player. The explorer will eventually assign a room to the player, but this can sometimes occur a couple of seconds after the scene is loaded.


## Get player platform

Players can access Decentraland via various platforms, including the official desktop app, and deprecated web and desktop versions, as well as [alternative experimental clients](https://github.com/decentraland/protocol-squad) built for other engines.

Use `getExplorerInformation()` to know what platform the current player is running Decentraland on.

```ts
import { getExplorerInformation } from '~system/Runtime';

executeTask(async () => {
  let data = await getExplorerInformation({})
  console.log("PLATFORM: ", data.platform, " AGENT: ", data.agent)
})
```

When using the official Decentraland desktop app, this function should return the following data:

```
{
    agent: unity-explorer,
    platform: desktop
}
```

Players using the official Decentraland desktop app are likely to have a much smoother experience than those on the browser, since the browser imposes performance limitations on how much of the machine's processing power the browser tab can use. They will also be missing many features like camera control, dynamic lights, freezing player movement, UI enhancements, etc.

## The EngineInfo Component

The `EngineInfo`component keeps track of data about the scene's lifecycle, which can sometimes be useful to tell when an event is occurring, relative to the scene's initialization.

This component is added to the `engine.RootEntity`.

```ts
engine.addSystem((deltaTime) => {
  const engineInfo = EngineInfo.getOrNull(engine.RootEntity)
  if (!engineInfo) return

  console.log(
    '--------------' +
      '\nframeNumber: ' +
      engineInfo.frameNumber +
      '\ntickNumber: ' +
      engineInfo.tickNumber +
      '\ntotalRuntime: ' +
      engineInfo.totalRuntime +
      '\nsceneHidden: ' +
      engineInfo.sceneHidden +
      '\n--------------'
  )
})
```

You can use `sceneHidden` to pause resource-intensive work while the scene is not visible:

```ts
engine.addSystem((deltaTime) => {
  const engineInfo = EngineInfo.getOrNull(engine.RootEntity)
  if (engineInfo?.sceneHidden) return

  // Normal gameplay logic runs only when the scene is visible
})
```

The `EngineInfo` component holds the following data:

* `frameNumber`: Frame counter of the engine.
* `totalRuntime`: Total runtime of this scene, in seconds.
* `tickNumber`: Tick counter of the scene as per [ADR-148](https://adr.decentraland.org/adr/ADR-148).
* `sceneHidden`: `true` when the scene is hidden behind the Explorer's fullscreen UI. That covers the loading screen, and also the map, the backpack, and the settings menu. Use this to mute audio and pause heavy work while the player can't see the scene.


> [!WARNING]
> **📔 Note**: The `EngineInfo` component must be imported via
> 
> > `import { EngineInfo } from "@dcl/sdk/ecs"`
> 
> See [Imports](/docs/creator/sdk7/getting-started/coding-scenes/#imports) for how to handle these easily.


### React to the loading screen fading out

The `scene_hidden` field tells you if the player can actually see your scene, or if it's covered by the Explorer's fullscreen UI. While the loading screen is up, `sceneHidden` is `true`. The moment the loading screen fades out and the player gets their first look at the world, it turns `false`.

It turns `true` again later whenever a fullscreen Explorer UI covers the scene, such as the map, the backpack, or the settings menu. That makes it a good cue to mute [audio](/docs/creator/sdk7/3d-essentials/sounds/) and pause expensive systems, then resume when it returns to `false`.

This is the only way for a scene to know when that first reveal happens. Use it to hold back anything that would otherwise play out behind the loading screen, and be missed by the player: intro cinematics, welcome sounds, a tween that only reads well if it's watched, an opening UI, or an analytics event that should only count once the player is really there.

```ts
import { engine, EngineInfo } from '@dcl/sdk/ecs'

function onSceneRevealed() {
  // The player is now looking at the scene, start the intro here
  console.log('The loading screen just faded out')
}

engine.addSystem(function waitForSceneRevealed() {
  const engineInfo = EngineInfo.getOrNull(engine.RootEntity)
  if (!engineInfo || engineInfo.sceneHidden) return

  // Only run once
  engine.removeSystem(waitForSceneRevealed)
  onSceneRevealed()
})
```


> [!WARNING]
> **📔 Note**: Your scene keeps running normally while `sceneHidden` is `true`, it's only not being displayed. Don't use this field to pause your scene's logic, use it to time what the player is meant to witness.
> 
> `scene_hidden` requires an up-to-date `@dcl/sdk` and a recent version of the Decentraland Explorer. On older clients the field stays at its default value of `false`, so a scene that waits on it still runs — it just won't be in sync with the loading screen fade-out.

