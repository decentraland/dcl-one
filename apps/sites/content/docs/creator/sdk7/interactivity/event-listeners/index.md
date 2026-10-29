> Events that the scene can track, related to player actions and scene changes.

# Event Listeners

There are several events that the scene can subscribe to, to know the actions of the player while in or near the scene.

For button and click events performed by the player, see [Button events](/docs/creator/sdk7/interactivity/button-events/click-events/).

## Player enters or leaves scene

Whenever an avatar steps inside or out of the parcels of land that make up your scene, or teleports in or out, this creates an event you can listen to.

This event is triggered by all avatars, including the player's.

```ts
import { onEnterScene, onLeaveScene } from '@dcl/sdk/src/players'

export function main() {
	onEnterScene((player) => {
		if (!player) return
		console.log('ENTERED SCENE', player)
	})

	onLeaveScene((userId) => {
		if (!userId) return
		console.log('LEFT SCENE', userId)
	})
}
```

On the `onEnterScene` event, the function can access all of the data returned by [get player data](/docs/creator/sdk7/interactivity/user-data/#get-player-data) via the `player` property.
On the `onLeaveScene` event, the function only has access to the player's ID.

### Only current player

You can filter out the triggered events to only react to the player's avatar, rather than other avatars that may be around.

```ts
import { getPlayer, onEnterScene, onLeaveScene } from '@dcl/sdk/src/players'

export function main() {
	let myPlayer = getPlayer()

	onEnterScene((player) => {
		if (!player) return
		console.log('ENTERED SCENE', player)

		if (myPlayer && player.userId == myPlayer.userId) {
			console.log('I CAME IN')
		}
	})

	onLeaveScene((userId) => {
		if (!userId) return
		console.log('LEFT SCENE', userId)

		if (myPlayer && userId == myPlayer.userId) {
			console.log('I LEFT')
		}
	})
}
```

This example first obtains the player's id, then subscribes to the events and compares the `userId` returned by the event to that of the player.

### Query all players in scene

Go over the full list of players who are currently on your scene by iterating over all entities with a `PlayerIdentityData` component.

```ts
import { engine, PlayerIdentityData, Transform } from '@dcl/sdk/ecs'

export function main() {
	for (const [entity, data, transform] of engine.getEntitiesWith(
		PlayerIdentityData,
		Transform
	)) {
		console.log('PLAYER: ', { entity, data, transform })
	}
}
```

## Player changes camera mode

Knowing the camera mode can be very useful to fine-tune the mechanics of your scene to better adjust to what's more comfortable using this mode. For example, small targets are harder to click when on 3rd person.

The following snippet uses the `onChange` function to fire an event each time the camera changes. It also fires an event when the scene loads, with the player's initial camera mode.

```ts
import { engine, CameraMode } from '@dcl/sdk/ecs'

export function main() {
	CameraMode.onChange(engine.CameraEntity, (cameraComponent) => {
		if (!cameraComponent) return
		console.log('Camera mode changed', cameraComponent?.mode)
		// 0 = first person
		// 1 = third person
	})
}
```

See [Check player's camera mode](/docs/creator/sdk7/interactivity/user-data/#check-the-players-camera-mode).

## Player plays animation

Use the `onChange` function on the `AvatarEmoteCommand` component to fire an event each time the player plays an emote. This includes both base emotes (dance, clap, wave, etc) and emotes from tokens.

```ts
import { AvatarEmoteCommand } from '@dcl/sdk/ecs'

export function main() {
	AvatarEmoteCommand.onChange(engine.PlayerEntity, (emote) => {
		if (!emote) return
		console.log('Emote played: ', emote.emoteUrn)
	})
}
```

The event includes the following information:

* `emoteUrn`: Name of the emote performed (ie: _wave_, _clap_, _kiss_)
* `loop`: If the emote is looping or playing once
* `timestamp`: When the emote was triggered.
* `state`: A value of the `EmoteState` enum describing the lifecycle event: `ES_STARTED` (the emote started; also the value when the field is absent, on older clients), `ES_FINISHED` (a non-looping emote played through to its end), or `ES_INTERRUPTED` (the emote was cut short by movement, teleport, another emote, an explicit stop, or leaving the scene). See [Detect when an emote finishes](/docs/creator/sdk7/interactivity/avatars/avatar-animations/#detect-when-an-emote-finishes).

You can also detect emotes from other players in the scene by passing a reference to the other player instead of `engine.PlayerEntity`.

## Player changes profile

Use the `onChange` function on the `AvatarEquippedData` component to fire an event each time the player changes one of their wearables, or their listed emotes on the quick access wheel. Similarly, use the `onChange` function on the `AvatarBase` to fire an event each time the player changes their base avatar properties, like hair color, skin color, avatar shape, or name.

```ts
import { AvatarEquippedData, AvatarBase } from '@dcl/sdk/ecs'

export function main() {
	AvatarEquippedData.onChange(engine.PlayerEntity, (equipped) => {
		if (!equipped) return
		console.log('New wearables list: ', equipped.wearableUrns)
		console.log('New emotes list : ', equipped.emoteUrns)
	})

	AvatarBase.onChange(engine.PlayerEntity, (body) => {
		if (!body) return
		console.log('New eyes color: ', body.eyesColor)
		console.log('New skin color: ', body.skinColor)
		console.log('New hair color: ', body.hairColor)
		console.log('New body URN: ', body.bodyShapeUrn)
	})
}
```

The event on `AvatarEquippedData` includes the following information:

* `wearableUrns`: The list of wearables that the player currently has equipped.
* `emoteUrns`: The list of emotes that the player currently has equipped in the quick access wheel.

The event on `AvatarBase` includes the following information:

* `name`: The player's name.
* `bodyShapeUrn`: The ids corresponding to male or female body type.
* `skinColor`: Player skin color as a `Color3`
* `eyesColor`: Player eye color as a `Color3`
* `hairColor`: Player hair color as a `Color3`

You can also detect changes in wearables or avatars from other players in the scene, simply pass a reference to the other player instead of `engine.PlayerEntity`.

You can also detect changes on the profiles of other players in the scene, simply pass a reference to the other player instead of `engine.PlayerEntity`.

## Player locks or unlocks cursor

Players can switch between two cursor modes: _locked cursor_ mode to control the camera or _unlocked cursor_ mode for moving the cursor freely over the UI.

Players unlock the cursor by clicking the _Right mouse button_ or pressing the _Esc_ key, and lock the cursor back by clicking anywhere in the screen.

Add a `PointerLock` component to the `engine.CameraEntity` entity in your scene, and use the `onChange` function on the `PointerLock` component to fire an event each time the player changes between the two cursor modes.

```ts
import {PointerLock} from '@dcl/sdk/ecs'

export function main() {
    PointerLock.onChange(engine.CameraEntity, (pointerLock) => {
		    if (!pointerLock) return
		    if(pointerLock.isPointerLocked){
			    // Show hint about cursor mode
		   }
	})
}
```

You can use this information to nudge the player subtly, like showing a UI popup saying that this game is better experienced with an unlocked cursor. Or you can also force the player's cursor mode by changing the `isPointerLocked` on the component. The following example always sets the cursor mode to unlocked:

```ts
import {PointerLock} from '@dcl/sdk/ecs'

export function main() {
    PointerLock.createOrReplace(engine.CameraEntity, {isPointerLocked: false});
    PointerLock.onChange(engine.CameraEntity, (pointerLock) => {
		    if (!pointerLock) return
		    if(pointerLock.isPointerLocked){
			    PointerLock.getMutable(engine.CameraEntity).isPointerLocked = false
		   }
	})
}
```

## Open explorer panels

Your scene can open the Explorer's built-in fullscreen panels (map, settings, backpack, etc.) using the `openExplorerUi` restricted action. This is useful for directing players to specific Explorer features from your scene's UI.

```ts
import { openExplorerUi } from '~system/RestrictedActions'
import { ExplorerUi } from '@dcl/sdk/ecs'

// Open the map panel
await openExplorerUi({ ui: ExplorerUi.EU_MAP })
```

The `ExplorerUi` enum has the following values:

* `EU_SETTINGS` (0)
* `EU_MAP` (1)
* `EU_BACKPACK` (2)
* `EU_CAMERA_REEL` (3)
* `EU_COMMUNITIES` (4)
* `EU_PLACES` (5)
* `EU_EVENTS` (6)


> [!WARNING]
> **Note:** Like other restricted actions (`movePlayerTo`, `triggerEmote`), `openExplorerUi` only works while the player is standing inside the scene's parcels.


### Observe when panels open or close

Use the `ExplorerUiEventsResult` component to detect when explorer panels are opened or closed. This is a grow-only component on `engine.RootEntity`, similar to `PointerEventsResult`. It streams events each time a panel opens or closes.

```ts
import { engine, ExplorerUiEventsResult, ExplorerUi } from '@dcl/sdk/ecs'

engine.addSystem(() => {
	// Iterate over new events since the last frame
	for (const event of ExplorerUiEventsResult.get(engine.RootEntity)) {
		if (event.event?.$case === 'opened') {
			console.log('Panel opened:', event.ui)
		}
		if (event.event?.$case === 'closed') {
			console.log('Panel closed:', event.ui)
		}

		// Check which specific panel
		if (event.ui === ExplorerUi.EU_MAP && event.event?.$case === 'closed') {
			console.log('Player closed the map')
		}
	}
})
```

Each event contains:

* `ui`: Which panel the event is about (an `ExplorerUi` value).
* `timestamp`: A monotonic counter for when the event occurred.
* `event`: Either `{ $case: 'opened' }` or `{ $case: 'closed' }`.


> [!NOTE]
> **💡 Tip**: For a working example of emote playback events, see the [`4,23-emote-finish`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/4,23-emote-finish) test scene, which logs every `AvatarEmoteCommand` entry appended to the player entity as `STARTED`, `FINISHED` or `INTERRUPTED` — play an emote out fully to see `FINISHED`, or walk away mid-playback to see `INTERRUPTED`.

