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
* `state`: A value of the `EmoteState` enum describing the lifecycle event: `ES_STARTED` (the emote started, and also the value when the field is absent), `ES_FINISHED` (a non-looping emote played through to its end), or `ES_INTERRUPTED` (the emote was cut short by movement, teleport, another emote, an explicit stop, or leaving the scene). See [Detect when an emote finishes](/docs/creator/sdk7/interactivity/avatars/avatar-animations/#detect-when-an-emote-finishes).

You can also detect emotes from other players in the scene by passing a reference to the other player instead of `engine.PlayerEntity`.


> [!NOTE]
> **💡 Tip**: For a working example of emote playback events, see the [`4,23-emote-finish`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/4,23-emote-finish) test scene, which logs every `AvatarEmoteCommand` entry appended to the player entity as `STARTED`, `FINISHED` or `INTERRUPTED` — play an emote out fully to see `FINISHED`, or walk away mid-playback to see `INTERRUPTED`.


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
* `talking`: _true_ while the player is speaking in voice chat, _false_ when they're silent, `undefined` when the client isn't reporting voice activity.

Because `talking` lives on `AvatarBase`, this callback also fires every time the player starts or stops speaking. If you only care about looks, compare the fields you need instead of reacting to every change. See [Detect when a player is speaking](/docs/creator/sdk7/interactivity/user-data/#detect-when-a-player-is-speaking).

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

Your scene can open the Explorer's built-in panels (map, settings, backpack, etc.) using the `openExplorerUi` restricted action. Most of them take over the screen, but not all of them do. This is useful for directing players to specific Explorer features from your scene's UI.

The player has to ask for it. Call `openExplorerUi` from inside a click handler, not on its own:

```ts
// Open the map when the player clicks a cube
import { openExplorerUi } from '~system/RestrictedActions'
import { engine, ExplorerUi, InputAction, MeshCollider, MeshRenderer, pointerEventsSystem, Transform } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

const mapButton = engine.addEntity()
Transform.create(mapButton, { position: Vector3.create(8, 1, 8) })
MeshRenderer.setBox(mapButton)
MeshCollider.setBox(mapButton)

pointerEventsSystem.onPointerDown(
	{ entity: mapButton, opts: { button: InputAction.IA_POINTER, hoverText: 'Open map' } },
	() => {
		void openExplorerUi({ ui: ExplorerUi.EU_MAP })
	}
)
```

The `ExplorerUi` enum has the following values:

* `EU_SETTINGS` (0)
* `EU_MAP` (1)
* `EU_BACKPACK` (2)
* `EU_CAMERA_REEL` (3)
* `EU_COMMUNITIES` (4)
* `EU_PLACES` (5)
* `EU_EVENTS` (6)
* `EU_ITEM_PURCHASE` (7): the item-purchase confirmation modal. The Explorer currently answers `REJECTED_FEATURE_DISABLED` for it.


> [!WARNING]
> **Note:** Like other restricted actions (`movePlayerTo`, `triggerEmote`), `openExplorerUi` only works while the player is standing inside the scene's parcels.


### Only a player action can open a panel

A panel opens only when the call is a direct response to something the player just did. A call that isn't, is refused.

**Works:**

* Clicking a 3D entity, with `pointerEventsSystem.onPointerDown` or `pointerEventsSystem.onPointerUp`.
* Clicking an element of your scene's UI, with `onMouseDown`.
* Pressing or releasing one of the activation buttons, `E`, `F` or `1` to `4`, detected with `inputSystem.isTriggered` and `PointerEventType.PET_DOWN` or `PointerEventType.PET_UP` (`IA_PRIMARY`, `IA_SECONDARY`, `IA_ACTION_3` to `IA_ACTION_6`).

**Does not work:**

* Calling from `main()`, so a panel can't greet the player on load.
* Calling from a timer, or after the player's action has passed.
* Calling after an `await` on something that takes time, such as a `fetch` or another restricted action. The player action only counts for about one frame. Do the work first, then open the panel.
* Hovering. `PET_HOVER_ENTER` and `PET_HOVER_LEAVE` are not player actions.
* Movement keys. Walking, running and jumping don't count, only the activation buttons listed above.

`openExplorerUi` resolves with an `openResult` that tells you what happened:

| Value                            | Meaning                                        |
| -------------------------------- | ---------------------------------------------- |
| `UNSPECIFIED` (0)                | No result was reported.                        |
| `OPENED` (1)                     | The panel opened.                              |
| `WAS_ALREADY_OPEN` (2)           | Another panel was already on screen. The panels share a single slot, so only one of them can be open at a time. |
| `REJECTED_NOT_CURRENT_SCENE` (3) | The player isn't standing in your scene.       |
| `REJECTED_FEATURE_DISABLED` (4)  | The Explorer has that panel turned off, or the `ui` value isn't a panel it knows. |
| `REJECTED_NO_USER_GESTURE` (5)   | The call wasn't a response to a player action. |


> [!WARNING]
> **Note:** If nothing opens and you see no error, check `openResult`. `REJECTED_NO_USER_GESTURE` (5) means your call ran outside a player action, usually because it sits in `main()`, in a timer, or after an `await`.


`WAS_ALREADY_OPEN` isn't only about your scene calling twice. The panels all occupy the same slot, so any one of them being on screen refuses any other, and the player may have opened that one by hand a moment earlier, with no involvement from your scene. Treat it as a normal outcome, not as a bug in your code. With `openExplorerUiAndWait()`, described below, the same situation arrives as a `notOpened` outcome.

### Wait until the player closes a panel

`openExplorerUiAndWait()` opens a panel and hands you a promise that resolves once the player closes it. Use it when your scene should carry on only after the player is done, for example after sending them to the backpack to put on a wearable.

```ts
import { openExplorerUiAndWait, ExplorerUi } from '@dcl/sdk/explorer-ui'

// Open the backpack, then continue once the player closes it
async function openBackpack() {
	try {
		const outcome = await openExplorerUiAndWait({ ui: ExplorerUi.EU_BACKPACK })

		if (outcome.$case === 'closed') {
			console.log('The player closed the backpack')
		}
	} catch (error) {
		console.log('The wait failed:', error)
	}
}
```

Call `openBackpack()` from a click handler, the same as `openExplorerUi`. The `await` happens inside the function, after the panel is requested, so the player action still counts.

The object you get back has a `$case` field that tells you how the wait ended:

* `closed`: the player closed the panel your scene opened. This is the normal outcome.
* `notOpened`: nothing opened. The `openResult` field says why, using the values in the table above.
* `timedOut`: the time limit you set ran out while the panel was still open.
* `matched`: an `until` condition ended the wait early, so the panel may still be on screen.

When the panel can't open, the promise resolves immediately with `notOpened`. Your scene never sits waiting for a panel that isn't there.

Those four outcomes cover every ordinary ending, but the promise can still reject: the request to the Explorer can fail outright, two channels can be registered under the same name, or an event can arrive that the helper can't attribute to any of the calls in flight. That's why the example above is wrapped in a `try`/`catch`. A rejection inside an `async` function called from a pointer handler is reported nowhere at all, so without the `catch` the wait simply never comes back and nothing tells you why.


> [!WARNING]
> **📔 Note**: `openExplorerUiAndWait()` requires `@dcl/sdk` version 7.30.0 or newer. It comes from the `@dcl/sdk/explorer-ui` module, which also re-exports `ExplorerUi`.


#### Set a time limit

There is no time limit by default. A player can leave a panel open for several minutes, and the promise stays pending all that time. Pass `timeoutMs` when your scene needs to move on regardless.

```ts
import { openExplorerUiAndWait, ExplorerUi } from '@dcl/sdk/explorer-ui'

// Stop waiting after 30 seconds, whether or not the map is still open
async function openMap() {
	const outcome = await openExplorerUiAndWait({ ui: ExplorerUi.EU_MAP }, { timeoutMs: 30000 })

	if (outcome.$case === 'timedOut') {
		console.log('The player is still looking at the map')
	}
}
```

#### Stop on an event instead

The `until` option ends the wait as soon as an event arrives on a channel, resolving with `matched` rather than `closed`. Channels are exported from the same module: `ExplorerUiEvents` for panel open and close events, and `ItemPurchase` for the result of a purchase flow. Add the same channel to the `collect` option if you also want the events themselves, which arrive in the outcome's `events` array.

Pass the channel on its own to stop on any of its events, or wrap it in `variant()` to stop on one kind of event only:

```ts
import { ExplorerUi, ExplorerUiEvents, openExplorerUiAndWait, variant } from '@dcl/sdk/explorer-ui'

// Carry on as soon as the backpack is on screen, without waiting for the player to close it
async function openBackpack() {
	const outcome = await openExplorerUiAndWait(
		{ ui: ExplorerUi.EU_BACKPACK },
		{ until: variant(ExplorerUiEvents, 'opened'), collect: [ExplorerUiEvents] }
	)

	if (outcome.$case === 'matched') {
		console.log('The backpack is open, collected', outcome.events.length, 'events')
	}
}
```

The `$case` values `variant()` accepts are the ones the channel's events carry: `opened` and `closed` on `ExplorerUiEvents`.

### Observe when panels open or close

Use the `ExplorerUiEventsResult` component to follow the panels your own scene opened. This is a grow-only component on `engine.RootEntity`, similar to `PointerEventsResult`. Each panel your scene opens with `openExplorerUi` adds one event when it appears, and another when it goes away, including when the player dismisses it with _Esc_.


> [!WARNING]
> **Note:** This isn't a scene-wide view of the Explorer's UI. A panel the player opens by hand, from the Explorer's own menus, produces no events here. A scene that never calls `openExplorerUi` never sees a single one.


`get()` hands you the whole set of events accumulated so far, up to the 100 most recent, not the ones that arrived since the last frame. A system that iterates it blindly reacts to every past event on every tick. Compare each event's `timestamp` against the last tick you handled:

```ts
import { engine, ExplorerUiEventsResult, ExplorerUi } from '@dcl/sdk/ecs'

let lastHandledTick = -1

engine.addSystem(() => {
	let newestTick = lastHandledTick

	for (const event of ExplorerUiEventsResult.get(engine.RootEntity)) {
		// The set keeps every event, so skip the ticks already handled
		if (event.timestamp <= lastHandledTick) continue
		if (event.timestamp > newestTick) newestTick = event.timestamp

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

	// Only after the loop, so several events on the same tick are all handled
	lastHandledTick = newestTick
})
```

`ExplorerUiEventsResult.onChange()` on `engine.RootEntity` is the other way to read the stream: the callback runs once per new event, so there's no bookkeeping to keep right.

Each event contains:

* `ui`: Which panel the event is about (an `ExplorerUi` value).
* `timestamp`: The scene tick the event happened on.
* `requestId`: The id your scene passed on the `openExplorerUi` call that opened this panel, or `0` if the call passed none. Use it to tell apart several calls in flight.
* `event`: Either `{ $case: 'opened' }` or `{ $case: 'closed' }`.


> [!NOTE]
> **💡 Tip**: For a working example of all of the above, see the [`80,-6-explorer-ui-events`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/80,-6-explorer-ui-events) test scene. It prints every verdict and every event to its own HUD, and covers the gesture gate, a panel already open, `requestId` correlation and each outcome of `openExplorerUiAndWait()`.

