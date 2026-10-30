> Obtain data from players as they interact with your scene.

# User Data

## Player position and rotation

Use the `PlayerEntity` and the `CameraEntity` to know the player's position and rotation, by checking their `Transform` components.

```ts
function getPlayerPosition() {
	if (!Transform.has(engine.PlayerEntity)) return
	if (!Transform.has(engine.CameraEntity)) return

	//player position
	const playerPos = Transform.get(engine.PlayerEntity).position

	//player rotation
	const playerRot = Transform.get(engine.PlayerEntity).rotation

	//camera position
	const CameraPos = Transform.get(engine.CameraEntity).position

	//camera rotation
	const CameraRot = Transform.get(engine.CameraEntity).rotation

	console.log('playerPos: ', playerPos)
	console.log('playerRot: ', playerRot)
	console.log('cameraPos: ', CameraPos)
	console.log('cameraRot: ', CameraRot)
}

engine.addSystem(getPlayerPosition)
```

* **PlayerEntity position**: The avatar's position, at chest height. Approximately at 0.88 meters above the ground.
* **PlayerEntity rotation**: The direction in which the avatar is facing, expressed as a quaternion.
* **CameraEntity position**:
  * In 1st person: Equal to the avatar's position, but at eye-level. Approximately at 1.75 meters above the ground.
  * In 3rd person: May vary depending on camera movements.
* **PlayerEntity rotation**:
  * In 1st person: Similar to the direction in which the avatar is facing, expressed as a quaternion. May be rounded slightly differently from the player's rotation.
  * In 3rd person: May vary depending on camera movements.


> [!WARNING]
> **📔 Note**: Avoid referring to the `engine.PlayerEntity` or the `engine.CameraEntity` on the initial scene loading, because that can result in errors if the entities are not initialized yet. To avoid this problem, use these inside the `main()` function, or on a function indirectly called by `main()`. You can also encapsulate the behavior in an async [`executeTask` block](/docs/creator/sdk7/programming-patterns/async-functions/#the-executetask-function).
> 
> Another option is to refer to these entities inside a system. There they will always be available, because the first execution of the system is called once the scene is already properly initialized.


## Fetch all players

All players in the scene have a `Transform` component. This component is read only in avatars. To fetch the positions of all players, [iterate over all entities with](/docs/creator/sdk7/interactivity/user-data/) a `PlayerIdentityData` component.

```ts
import { PlayerIdentityData } from '@dcl/sdk/ecs'

for (const [entity, data, transform] of engine.getEntitiesWith(
	PlayerIdentityData,
	Transform
)) {
	console.log('Player data: ', { entity, data, transform })
}
```

The code above iterates over all entities with a `Transform` and a `PlayerIdentityData` component, and logs their data. You can use this same method to get any of the available data of all players.

See [Event listeners](/docs/creator/sdk7/interactivity/event-listeners/#player-locks-or-unlocks-cursor) to learn how to detect and react when new players join into the scene.

## Get player data

Use `getPlayer()` to fetch data about the current player, or any other player in the scene.

```ts
import { getPlayer } from '@dcl/sdk/src/players'

export function main() {
	createCube(5, 1, 5)

	let myPlayer = getPlayer()

	if (myPlayer) {
		console.log('Name : ', myPlayer.name)
		console.log('UserId : ', myPlayer.userId)
	}
}
```

`getPlayer()` returns the following:

* `name`: _(string)_ The player's user name, as others see in-world
* `userId`: _(string)_ A string that identifies the player. For players connected with a wallet, this is the wallet's address, written in lowercase. For guest accounts, it's a locally-generated identifier.
* `isGuest`: _(boolean)_ Indicates if the player has a public key. _True_ if the player is a guest account without a public key.
* `position`: _(Vector3)_ The position of the avatar in the scene.
* `avatar`: A nested object with data about the player's base avatar and appearance.
* `wearables`: An array of identifiers for each of the wearables that the player is currently wearing. For example `urn:decentraland:off-chain:base-avatars:green_hoodie`. All wearables have a similar identifier, even if they're NFTs.
* `emotes`: An array of identifiers for each of the emotes that the player currently has equipped in the quick access wheel.
* `entity`: A reference to the player entity. This can be handy to pass to other functions, or to add custom components to it.

The `avatar` object has the following nested information:

* `bodyShapeUrn`: An identifier for the avatar's general body shape. Either `urn:decentraland:off-chain:base-avatars:BaseFemale` for female or `urn:decentraland:off-chain:base-avatars:BaseMale` for male.
* `skinColor`: Player skin color as a `Color3`
* `eyesColor`: Player eye color as a `Color3`
* `hairColor`: Player hair color as a `Color3`
* `name`: The player's name.


> [!WARNING]
> **📔 Note**: The player data may not be available on the first frame of the scene, depending on load times. You should verify that the data was returned and otherwise attempt again a few milliseconds later.


```ts
import { getPlayer } from '@dcl/sdk/src/players'

export function main() {
	createCube(5, 1, 5)

	let myPlayer = getPlayer()

	if (myPlayer) {
		console.log('Is Guest: ', myPlayer.isGuest)
		console.log('Name : ', myPlayer.name)
		console.log('UserId : ', myPlayer.userId)
		console.log('Avatar shape : ', myPlayer.position)
		console.log('Avatar shape : ', myPlayer.avatar?.bodyShapeUrn)
		console.log('Avatar eyes color : ', myPlayer.avatar?.eyesColor)
		console.log('Avatar hair color : ', myPlayer.avatar?.hairColor)
		console.log('Wearables on : ', myPlayer.wearables)
		console.log('Emotes available : ', myPlayer.emotes)
	}
}
```

To get the data for a specific player in the scene, different from the current player, run `getPlayer()` with an object with a `userId` property.

```ts
import { getPlayer } from '@dcl/sdk/src/players'

for (const [entity, data, transform] of engine.getEntitiesWith(
	PlayerIdentityData,
	Transform
)) {
	let player = getPlayer({ userId: data.address })
	console.log('PLAYER : ', player?.name)
}
```

The snippet above iterates over all the entities with a `PlayerIdentityData` component, meaning all the avatar entities in the scene. It then runs the `getPlayer()` for that entity.

`getPlayer()` can only fetch data from players who are currently standing in the same scene, they don't have to necessarily be in visual range, but they should be connected to the same comms island. To try this out in preview, open a second tab and log in with a different account, and have both players stand inside the scene.


> [!WARNING]
> **📔 Note**: User IDs must always be lowercase. If copying a wallet address, make sure all the characters are set to lowercase.


## Data from any player

To obtain information from any player, make a [REST API call](/docs/creator/sdk7/networking/network-connections/#call-a-rest-api) to the content servers.

This information is exposed in the following URL, appending the player's user id to the url parameter.

`https://peer.decentraland.org/lambdas/profile/<player user id>`


> [!NOTE]
> **💡 Tip**: Try the URL out in a browser to see how the response is structured.


The following information is available from this API:

* `displayName`: _(string)_ The player's user name, as others see in-world
* `userId`: _(string)_ A UUID string that identifies the player. If the player has a public key, this field will have the same value as the public key.
* `hasConnectedWeb3`: _(boolean)_ Indicates if the player has a public key. _True_ if the player has one.
* `publicKey`: _(string)_ The public key of the player's Ethereum wallet. If the player logs in as a guest, with no linked wallet, this field will be `null`.
* `avatar`: A nested object with data about the player's appearance.
* `version`: _(number)_ A version number that increases by one every time the player changes any of their settings. Use this if you encounter conflicting data, to know what version is more recent.


> [!WARNING]
> **📔 Note**: For any Ethereum transactions with the player, always use the `publicKey` field, instead of the `userId`, to avoid dealing with non-existing wallets.


The `avatar` object has the following nested information:

* `wearables`: `WearableId[]` An array of identifiers for each of the wearables that the player is currently wearing. For example `urn:decentraland:off-chain:base-avatars:green_hoodie`. All wearables have a similar identifier, even if they're NFTs.
* `bodyShape`: An identifier for the avatar's general body shape. Either `urn:decentraland:off-chain:base-avatars:BaseFemale` for female or `urn:decentraland:off-chain:base-avatars:BaseMale` for male.
* `skin`: An object with a `color` field, holding the player's skin color as `{ r, g, b, a }` numbers from 0 to 1.
* `hair`: An object with a `color` field, holding the player's hair color as `{ r, g, b, a }` numbers from 0 to 1.
* `eyes`: An object with a `color` field, holding the player's eye color as `{ r, g, b, a }` numbers from 0 to 1.
* `snapshots`: A nested object with base64 representations of .jpg images of the player in various resolutions.
  * `face256`: _string_ The player's face as a 256x256 pixel image.
  * `body`: _string_ The full resolution image of the player standing straight, with 512x1024 pixels.


> [!CAUTION]
> **❗Warning** The snapshots of the avatar will be deprecated in the future and will no longer be returned as part of an avatar's data. The recommended approach is to use `AvatarTexture` instead, see [Avatar Portraits](/docs/creator/sdk7/3d-essentials/materials/#avatar-portraits).


Unlike `getPlayer()`, this option is not limited to just the players who are currently in the same scene, or even in the same server. With this approach you can fetch data from any player that has logged onto the servers in the past.

If you know which server the player you want to query is connected to, you can get more up-to-date data by sending your requests to that specific server. For example, if the player changes clothes, this information will be available instantly in the player's server, but will likely take a couple of minutes to propagate to the `peer.decentraland.org` server.

`https://<player server>/lambdas/profile/<player user id>`


> [!NOTE]
> **💡 Tip**: You can obtain the current player's server by calling `getRealm()` and reading the `realmInfo.baseUrl` field of the response.


This example combines `myProfile.userId` and `getRealm()` to obtain the player's data directly from the server that the player is on:

```ts
import { getRealm } from '~system/Runtime'
import { myProfile } from '@dcl/sdk/network'

async function fetchPlayerData() {
	const { realmInfo } = await getRealm({})
	if (!realmInfo) return

	const url = `${realmInfo.baseUrl}/lambdas/profile/${myProfile.userId}`
	console.log('using URL: ', url)

	try {
		const response = await fetch(url)
		const json = await response.json()

		console.log('full response: ', json)
		console.log('player is wearing :', json.avatars[0].avatar.wearables)
	} catch {
		console.log('an error occurred while reaching for player data')
	}
}

fetchPlayerData()
```

## Player data components

Instead of using `getPlayer()`, you can read data directly from a series of components that store the data on each player entity. The following components exist:

* `PlayerIdentityData`: Stores the player address and an `isGuest` property to flag guest accounts.
* `AvatarBase`: Stores data about the base avatar, including:
  * `name`: The player's name.
  * `bodyShapeUrn`: The ids corresponding to male or female body type.
  * `skinColor`: Player skin color as a `Color3`
  * `eyesColor`: Player eye color as a `Color3`
  * `hairColor`: Player hair color as a `Color3`
* `AvatarEquippedData`: The list of equipped wearables and emotes.
  * `wearableUrns`: The list of wearables that the player currently has equipped.
  * `emoteUrns`: The list of emotes that the player currently has equipped in the quick access wheel.
* `AvatarEmoteCommand`: Info about emotes that the player is currently playing. It includes:
  * `emoteUrn`: The URN for the last emote played by the player, since they entered the scene
  * `loop`: True if the emote is being looped
  * `timestamp`: The time when this emote was triggered

```ts
for (const [entity, data, base, attach, transform] of engine.getEntitiesWith(
	PlayerIdentityData,
	AvatarBase,
	AvatarEquippedData,
	Transform
)) {
	console.log('PLAYER DATA: ', { entity, data, transform, base, attach })
}
```


> [!WARNING]
> **📔 Note**: All of these components are read-only. You cannot change their values from the scene.


## Get Portable Experiences

Portable experiences are essentially scenes that are not constrained to parcels of land. Players can carry these with them anywhere they go in Decentraland, adding a new layer of content over the world. Smart Wearables are examples of portable experiences. You may want to know if a player is wearing one of these, since a smart wearable may enable players to have abilities that could be considered cheating in a competitive game. For example, in a platform game, a player that wears a jetpack has a very unfair advantage over others.

As a scene creator, you may want to limit what players wearing portable experiences can do in your scene. Use `getPortableExperiencesLoaded()` to check if the player has any portable experiences currently activated.

```ts
import { getPortableExperiencesLoaded } from '~system/PortableExperiences'

executeTask(async () => {
	let portableExperiences = await getPortableExperiencesLoaded({})
	console.log(portableExperiences.loaded)
})
```

`getPortableExperiencesLoaded()` returns an array of objects, each of these objects includes an `id` attribute. In the case of wearables, the id is the wearable's URN.

## Get detailed info about a player's wearables

The `getPlayer()` function returns only a list of wearable ids, without information about each wearable. Maybe you want to check for any wearable of a specific category (eg: hats), or any wearable of a specific rarity (eg: Mythic), for that you'll need to fetch more detailed information about the player's wearables.

Make a [REST API call](/docs/creator/sdk7/networking/network-connections/#call-a-rest-api) to the following URL, to obtain a full updated list of all wearables that are currently usable, with details about each.

`${playerRealm.realmInfo.baseUrl}/lambdas/collections/wearables-by-owner/${userData.userId}?includeDefinitions`


> [!WARNING]
> **📔 Note**: To construct this URL, you must obtain the realm (likely with with `getRealm()`) and the player's id (likely with `getPlayer()`)


This feature could be used together with fetching info about the player, to for example only allow players to enter a place if they are wearing any wearable from the halloween collection, or any wearable that is of _legendary_ rarity.


> [!NOTE]
> **💡 Tip**: Try the URL out in a browser to see how the response is structured.


```ts
import { getPlayer } from '@dcl/sdk/src/players'
import { getRealm } from '~system/Runtime'

async function fetchWearablesData() {
	try {
		let userData = getPlayer()
		const realm = await getRealm({})
		if (!userData || !realm.realmInfo) return

		const url =
			`${realm.realmInfo?.baseUrl}/lambdas/collections/wearables-by-owner/${userData.userId}?includeDefinitions`.toString()
		console.log('using URL: ', url)

		let response = await fetch(url)
		let json = await response.json()

		console.log('full response: ', json)
	} catch {
		console.log('an error occurred while reaching for wearables data')
	}
}

executeTask(fetchWearablesData)
```


> [!NOTE]
> **💡 Tip**: You can fetch even more info about specific wearables from the [following API](https://decentraland.github.io/catalyst-api-specs/#tag/Lambdas/operation/searchWearables).


## Check the player's camera mode

Players can either be using a 1st or 3rd person camera when exploring Decentraland. Check which of these the player is using by checking the value `CameraMode` component of the `engine.CameraEntity` entity.

```ts
function checkCameraMode() {
	if (!CameraMode.has(engine.CameraEntity)) return

	let cameraEntity = CameraMode.get(engine.CameraEntity)

	if (cameraEntity.mode == CameraType.CT_THIRD_PERSON) {
		console.log('The player is using the 3rd person camera')
	} else {
		console.log('The player is using the 1st person camera')
	}
}

engine.addSystem(checkCameraMode)
```


> [!WARNING]
> **📔 Note**: Camera information is only available for the current player running the scene. You can't query for the camera data of any other player.


The camera mode uses a value from the `CameraType` enum. The following values are possible:

* `CameraType.CT_FIRST_PERSON`
* `CameraType.CT_THIRD_PERSON`

The `CameraMode` component of the `engine.CameraEntity` is read-only, you can't force the player to change camera mode through this.


> [!NOTE]
> **💡 Tip**: To change the player's camera mode, use a [Camera modifier area](/docs/creator/sdk7/3d-essentials/camera/#1st-and-3rd-person-camera-modes).


Knowing the camera mode can be very useful to fine-tune the mechanics of your scene to better adjust to what's more comfortable using this mode. For example, small targets are harder to click when in 3rd person.


> [!WARNING]
> **📔 Note**: Avoid referring to the `engine.CameraEntity` on the initial scene loading, because that can result in errors if the entities are not initialized yet. To avoid this problem, use these inside the `main()` function, or on a function indirectly called by `main()`. You can also encapsulate the behavior in an async [`executeTask` block](/docs/creator/sdk7/programming-patterns/async-functions/#the-executetask-function).
> 
> Another option is to refer to this entity inside a system. It will always be available, because the first execution of the system is called once the scene is already properly initialized.


## Check if the player has the cursor locked

Players can switch between two cursor modes: _locked cursor_ mode to control the camera or _unlocked cursor_ mode for moving the cursor freely over the UI.

Players unlock the cursor by clicking the _Right mouse button_ or pressing the _Esc_ key, and lock the cursor back by clicking anywhere in the screen.

Check the `PointerLock` component of the scene's [camera entity](/docs/creator/sdk7/architecture/entities-components/#reserved-entities) to find out what the current cursor mode is.

```ts
export function main() {
	const isLocked = PointerLock.get(engine.CameraEntity).isPointerLocked
	console.log(isLocked)
}
```

See [Event listeners](/docs/creator/sdk7/interactivity/event-listeners/#player-locks-or-unlocks-cursor) to see how to easily react to changes in the cursor state.

You can also force the player's cursor state by writing to the `isPointerLocked` field of the `PointerLock` component on `engine.CameraEntity`. See [Lock or unlock the cursor](/docs/creator/sdk7/interactivity/button-events/click-events/#lock-or-unlock-the-cursor) for examples.


> [!WARNING]
> **📔 Note**: Avoid referring to the `engine.CameraEntity` on the initial scene loading, because that can result in errors if the entities are not initialized yet. To avoid this problem, use these inside the `main()` function, or on a function indirectly called by `main()`. You can also encapsulate the behavior in an async [`executeTask` block](/docs/creator/sdk7/programming-patterns/async-functions/#the-executetask-function).
> 
> Another option is to refer to the entity inside a system. It will always be available, because the first execution of the system is called once the scene is already properly initialized.


## Check the player's cursor position

Use the `PrimaryPointerInfo` component on the `engine.RootEntity` to get the player's cursor position. This can be used for mechanics like drag and drop interactions, swipe gestures, etc.

```ts
import { PrimaryPointerInfo } from '@dcl/sdk/ecs'

function CursorSystem() {
	const pointerInfo = PrimaryPointerInfo.get(engine.RootEntity)
	console.log(pointerInfo)
}

engine.addSystem(CursorSystem)
```


> [!WARNING]
> **📔 Note**: Avoid referring to the `engine.RootEntity` on the initial scene loading, because that can result in errors if the entities are not initialized yet. To avoid this problem, always refer to the entity inside a system. It will always be available, because the first execution of the system is called once the scene is already properly initialized.


The `PrimaryPointerInfo` component returns an object with the following properties:

* `screenCoordinates`: _(Vector2)_ The position of the cursor in the scene, expressed in pixels. The origin is the bottom left corner of the screen. When the cursor is locked, this reports the center of the screen.
* `screenDelta`: _(Vector2)_ The change in the position of the cursor since the last frame, expressed in pixels. See [Mouse Movement](/docs/creator/sdk7/interactivity/mouse-movement/) for details and examples.
* `worldRayDirection`: _(Vector3)_ A vector that represents the direction of the ray from the camera to the cursor. The origin is the camera position. Use this to calculate the position of the cursor in the world.
* `pointerType`: 0 for `none`, 1 for `mouse`


> [!NOTE]
> **💡 Tip**: Unlike the other properties, `screenDelta` keeps reporting mouse movement even while the cursor is [locked](/docs/creator/sdk7/interactivity/button-events/click-events/#lock-or-unlock-the-cursor). This makes it ideal for real-time interactions like drag gestures and custom camera controls, see [Mouse Movement](/docs/creator/sdk7/interactivity/mouse-movement/).



> [!NOTE]
> **Tip:** To react to simple hover events on UI elements, you may find it easier to use the `onMouseEnter` and `onMouseLeave` events, see [UI Button Events](/docs/creator/sdk7/2d-ui/ui_button_events/#hover-feedback).


The `PrimaryPointerInfo` component is read-only, you can't force the player to change the cursor position.

The following example shows how to display the cursor position on a UI element.

_**ui.tsx file:**_

```tsx
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'
import {cursorXpos, cursorYpos} from './index'

export const uiMenu = () => (
  <UiEntity
    uiTransform={{
			width: '100%',
			height: '100px',
			justifyContent: 'center',
			alignItems: 'center',
    }}
    uiText={{ value: `Cursor pos: `+  cursorXpos + `,` + cursorYpos, fontSize: 40 }}
    uiBackground={{ color: Color4.create(0.5, 0.8, 0.1, 0.6) }}
  />
)
```

_**index.ts file:**_

```ts
import { engine, PrimaryPointerInfo } from '@dcl/sdk/ecs'
import { ReactEcsRenderer } from '@dcl/sdk/react-ecs'
import { uiMenu } from './ui'

export function main() {
  ReactEcsRenderer.setUiRenderer(uiMenu)
}

export let cursorXpos: number | undefined = undefined
export let cursorYpos: number | undefined = undefined

function CursorSystem() {
  const pointerInfo = PrimaryPointerInfo.get(engine.RootEntity)
  console.log(pointerInfo)

  cursorXpos = pointerInfo.screenCoordinates?.x
  cursorYpos = pointerInfo.screenCoordinates?.y
}

engine.addSystem(CursorSystem)
```

You can use the `worldRayDirection` to set the `direction` field of a raycast to know if an entity is in the cursor's line of sight. See [Raycasting](/docs/creator/sdk7/interactivity/raycasting/) for more details.
