> Learn how to handle user clicks in your scene.

# System Based Events

If your scene has multiple similar entities that are all activated using the same logic, you can write a single system to iterates over all of them and describe that behavior only once. This is also the most performant and more [data oriented](/docs/creator/sdk7/architecture/data-oriented-programming/) approach.

You can also use a system to detect [global input events](/docs/creator/sdk7/interactivity/button-events/system-based-events/#global-input-events), so that the scene reacts whenever a key is pressed, without consideration for where the player's cursor is aiming.

If all you want to do is click or push a button on a single entity to activate it, the easiest way is to use the [Register a callback](/docs/creator/sdk7/interactivity/button-events/register-callback/) approach.

To set more specific custom logic, you might want to deal with the raw data and use the [Advanced](/docs/creator/sdk7/interactivity/button-events/advanced-button-events/) approach.

For an entity to be interactive, it must have a [collider](/docs/creator/sdk7/3d-essentials/colliders/). See [obstacles](/docs/creator/sdk7/interactivity/button-events/click-events/#obstacles) for more details. This also applies to proximity click events.

## Using a system

Check for button events by running one of the helper functions on the input `inputSystem` namespace on every tick within a [system](/docs/creator/sdk7/architecture/systems/).

For example, the following system uses the `inputSystem.isTriggered()` function to check if the pointer was clicked. On every tick, it checks if the button was pressed. If `inputSystem.isTriggered()` returns _true_, the system runs some custom logic in response.

```ts
engine.addSystem(() => {
	if (inputSystem.isTriggered(InputAction.IA_POINTER, PointerEventType.PET_DOWN)) {
		// Logic in response to button press
	}
})
```

The following helper functions are available in the `inputSystem` namespace, and can be called similarly on every tick:

* `inputSystem.isTriggered`: Returns true if an input action ocurred since the last tick.
* `inputSystem.isPressed`: Returns true if an input is currently being pressed down. It will return true on every tick until the button goes up again.
* `inputSystem.getInputCommand`: Returns an object with data about the input action.

See the sections below for more details on each.

When handling button events on an entity, always provide feedback to the player, so that the player is aware that an entity can be interacted with. If you add a `PointerEvents` component to an entity, players will see a hint while hovering their cursor on that entity. See [Show feedback](/docs/creator/sdk7/interactivity/button-events/system-based-events/#show-feedback) to learn how you can add hover hints on interactive entities.

### Global input events

Use `inputSystem.isTriggered` to detect button down and button up events on any of the inputs tracked by the SDK.

```ts
engine.addSystem(() => {
	if (
		inputSystem.isTriggered(InputAction.IA_POINTER, PointerEventType.PET_DOWN)
	) {
		// Logic in response to button press
	}
})
```

The example above checks if the button was pressed, regardless of where the pointer is at, or what entities may be in its path.

`inputSystem.isTriggered()` returns _true_ only if the indicated button was pressed down in the current tick of the game loop. If the button was not pushed down, or it was already down from the previous tick, it returns _false_.


> [!WARNING]
> **📔 Note**: The player needs to be standing inside the scene's boundaries for the pointer event to be detected. The player's cursor also needs to be locked, buttons pressed while having the free cursor aren't detected.
> 
> Every entity you want to inteact with must have both a [`PointerEvents` component](/docs/creator/sdk7/interactivity/button-events/system-based-events/#show-feedback) and a [collider](/docs/creator/sdk7/3d-essentials/colliders/).


The `inputSystem.isTriggered` function takes the following required arguments:

* `InputAction`: Which input to listen for, as a value from the `InputAction` enum. See [Pointer buttons](/docs/creator/sdk7/interactivity/button-events/click-events/#pointer-buttons) for supported options.
* `PointerEventType`: What type of event to listen for, as a value from the `PointerEventType` enum. See [Types of pointer events](/docs/creator/sdk7/interactivity/button-events/click-events/#types-of-pointer-events) for supported options.

### Activate an entity

To detect button events while pointing at a particular entity, pass a third optional argument to `inputSystem.isTriggered` to specify what entity to check against.

```ts
// Give entity a PointerEvents component
PointerEvents.create(myEntity, {
	pointerEvents: [
		{
			eventType: PointerEventType.PET_DOWN,
			eventInfo: {
				button: InputAction.IA_PRIMARY,
				showFeedback: false,
			},
		},
	],
})

// create system
engine.addSystem(() => {
	if (
		inputSystem.isTriggered(
			InputAction.IA_POINTER,
			PointerEventType.PET_DOWN,
			myEntity
		)
	) {
		// Logic in response to button press on myEntity
	}
})
```

The example above checks on every tick if a single hard-coded entity was pressed with the pointer button (left mouse button).

The `inputSystem.isTriggered` function takes the following arguments:

* `InputAction`: Which input to listen for, as a value from the `InputAction` enum. See [Pointer buttons](/docs/creator/sdk7/interactivity/button-events/click-events/#pointer-buttons) for supported options.
* `PointerEventType`: What type of event to listen for, as a value from the `PointerEventType` enum. See [Types of pointer events](/docs/creator/sdk7/interactivity/button-events/click-events/#types-of-pointer-events) for supported options.
* `Entity` _(optional)_: What entity to check these events on. If no value is provided, it will check for global presses of the button, regardless of where the player's cursor was pointing at.

Note that in this example we're also adding a `PointerEvents` component to the entity we want to interact with. This step is necessary, without this component the entity won't be detectable by any of the functions of `inputSystem`. see [Show Feedback](/docs/creator/sdk7/interactivity/button-events/system-based-events/#show-feedback) for more details on the `PointerEvents` component.


> [!WARNING]
> **📔 Note**: and a [collider](/docs/creator/sdk7/3d-essentials/colliders/). See [obstacles](/docs/creator/sdk7/interactivity/button-events/click-events/#obstacles) for more details.


If there are multiple entities that the player can interact with in the same way, consider using `inputSystem.getInputCommand`. This command returns infor about a global click command, including the entity ID, you can use this to execute a single function that can handle them all.

```ts
engine.addSystem(() => {
	const result = inputSystem.getInputCommand(
		InputAction.IA_POINTER,
		PointerEventType.PET_DOWN
	)
	if (result) {
		if (result.hit?.entityId === myEntity) {
			// handle click
		}
	}
})
```


> [!WARNING]
> **📔 Note**: Every entity you want to inteact with must have both a [`PointerEvents` component](/docs/creator/sdk7/interactivity/button-events/system-based-events/#show-feedback) and a [collider](/docs/creator/sdk7/3d-essentials/colliders/).


See [Data from input action](/docs/creator/sdk7/interactivity/button-events/system-based-events/#data-from-input-action) for more info. This method also grants you more detailed data about the hit of the pointer event.

### Input button up

You can also check for pointer up events in much the same way, by using `PointerEventType.PET_UP`.

```ts
engine.addSystem(() => {
	if (
		inputSystem.isTriggered(
			InputAction.IA_POINTER,
			PointerEventType.PET_UP,
			myEntity
		)
	) {
		// Logic in response to button up while pointing at myEntity
	}
})
```


> [!WARNING]
> **📔 Note**: When checking pointer up events against a specific entity, it doesn't take into consideration where the cursor was pointing at when the button was pushed down. It only considers where the cursor is pointing at when the button is raised.
> 
> Also keep in mind that hte entity must have both a [`PointerEvents` component](/docs/creator/sdk7/interactivity/button-events/system-based-events/#show-feedback) and a [collider](/docs/creator/sdk7/3d-essentials/colliders/).


### Check for pressed buttons

Check if a button is currently being pressed down by using `inputSystem.isPressed()` within a system.

```ts
engine.addSystem(() => {
	if (inputSystem.isPressed(InputAction.IA_POINTER)) {
		// Logic in response to button being held down
	}
})
```

`inputSystem.isPressed()` returns _true_ if the button is currently being held down, no matter when the button was pushed down, and no matter where the player's cursor is pointing at. Otherwise it returns _false_.

The `inputSystem.isPressed` function takes a single argument:

* `InputAction`: Which input to listen for, as a value from the `InputAction` enum. See [Pointer buttons](/docs/creator/sdk7/interactivity/button-events/click-events/#pointer-buttons) for supported options.

### Handle multiple entities

If your scene has multiple entities that are affected by pointer events in the same way, it makes sense to write a system that iterates over all of them.

```ts
engine.addSystem(() => {
	const activatedEntites = engine.getEntitiesWith(PointerEvents)
	for (const [entity] of activatedEntites) {
		if (
			inputSystem.isTriggered(
				InputAction.IA_POINTER,
				PointerEventType.PET_DOWN,
				entity
			)
		) {
			// Logic in response to interacting with an entity
		}
	}
})
```

This example uses a [component query](/docs/creator/sdk7/architecture/querying-components/) to iterate over all the entities with a `PointerEvents` component. It then checks each of these entities with `inputSystem.isTriggered`, iterating over them one by one. If an input action is detected on any of these entities, it carries out custom logic.

Instead of iterating over _all_ the entities with a `PointerEvents` component in a single system, you might want to write different systems to handle entities that should behave in different ways. The recommended approach is to mark different types of entities with specific [custom components](/docs/creator/sdk7/architecture/custom-components/), and iterate over them in separate systems.

```ts
engine.addSystem(() => {
	const activatedDoors = engine.getEntitiesWith(IsDoor, PointerEvents)
	for (const [entity] of activatedDoors) {
		if (
			inputSystem.isTriggered(
				InputAction.IA_POINTER,
				PointerEventType.PET_DOWN,
				entity
			)
		) {
			openDoor(entity)
		}
	}
})

engine.addSystem(() => {
	const pickedUpGems = engine.getEntitiesWith(IsGem, PointerEvents)
	for (const [entity] of pickedUpGems) {
		if (
			inputSystem.isTriggered(
				InputAction.IA_POINTER,
				PointerEventType.PET_DOWN,
				entity
			)
		) {
			pickUp(entity)
		}
	}
})
```

This example has one system that iterates over all entities that have a custom component named `IsDoor` and another that iterates over all entities that have a custom component named `isGem`. In both systems, it checks every matching entity to see if they were activated with the pointer button.

This way of organizing your scene's code is very [data oriented](/docs/creator/sdk7/architecture/data-oriented-programming/) and should result in a very efficient use of memory resources.

## Multiple buttons on an entity

Using helpers like `pointerEventsSystem.onPointerDown`, each entity is limited to have a single type of event. You can't register two different button events. You don't have that limitation when using sytem based events. For example here's an entity that players can interact with both by pressing E and F, each triggering different functionalities.


```ts
    engine.addSystem(() => {
        if (inputSystem.isTriggered(InputAction.IA_PRIMARY, PointerEventType.PET_DOWN, myEntity)) {
            // react to E
        }
        if (inputSystem.isTriggered(InputAction.IA_SECONDARY, PointerEventType.PET_DOWN, myEntity)) {
            // react to F
        }
    })
```


## Show feedback

To display UI hints while pointing at an entity, use the properties in the entity's `PointerEvents` component.

```ts
// create entity
const myEntity = engine.addEntity()

// give entity a PointerEvents component
PointerEvents.create(myEntity, {
	pointerEvents: [
		{
			eventType: PointerEventType.PET_DOWN,
			eventInfo: {
				button: InputAction.IA_POINTER,
			},
		},
	],
})
```

Whenever the player's cursor points at the colliders in this entity, the UI will display a hover hint to indicate that the entity can be interacted with. See the sections below for details on what you can configure.


> [!WARNING]
> **📔 Note**: The `PointerEvents` component just handles the displaying of hover feedback. To handle the button events themselves with custom logic, see [Using-a-system](/docs/creator/sdk7/interactivity/button-events/system-based-events/#check-for-events).


The `PointerEvents` component requires at least one pointer event definition. Each pointer event definition can be configured with the following:

* `eventType`: What type of event to listen for, as a value from the `PointerEventType` enum. See [Types of pointer events](/docs/creator/sdk7/interactivity/button-events/click-events/#types-of-pointer-events) for supported options.
* `eventInfo`: An object that can contain the following fields:
  * `button` (_required_): Which input to listen for, as a value from the `InputAction` enum. See [Pointer buttons](/docs/creator/sdk7/interactivity/button-events/click-events/#pointer-buttons) for supported options.
  * `hoverText` _(optional)_: What string to display in the hover feedback hint. "Interact" by default.
  * `showFeedback` _(optional)_: If false, it hides both the hover hint and the edge highlight for this entity. _true_ by default.
  * `showHighlight` _(optional)_: If true, players will see the edge highlight when hovering the cursor on the entity. _true_ by default. This value is only considered if `showFeedback` is _true_.
  * `maxDistance` _(optional)_: How far the player's **avatar** can be from the entity and still interact with it, in meters. Default is _10 meters_. See [Max distance](#max-distance).
  * `maxCameraDistance` _(optional)_: How far the **active camera** can be from the entity and still interact with it, in meters. Not set by default. See [Max distance](#max-distance).
  * `priority` _(optional)_: Which entity wins when several overlap. Higher values take precedence. _0_ by default.

A single `PointerEvents` component can hold multiple pointer events definitions, that can detect different events for different buttons. Each entity can only have _one_ `PointerEvents` component, but this component can include multiple objects in its `pointerEvents` array, one for each event to respond to.

```ts
PointerEvents.create(NPCEntity, {
	pointerEvents: [
		{
			eventType: PointerEventType.PET_DOWN,
			eventInfo: {
				button: InputAction.IA_POINTER,
				hoverText: 'Greet',
			},
		},
		{
			eventType: PointerEventType.PET_DOWN,
			eventInfo: {
				button: InputAction.IA_PRIMARY,
				hoverText: 'Choke',
			},
		},
		{
			eventType: PointerEventType.PET_UP,
			eventInfo: {
				button: InputAction.IA_PRIMARY,
				hoverText: 'Stop Choke',
			},
		},
		{
			eventType: PointerEventType.PET_DOWN,
			eventInfo: {
				button: InputAction.IA_SECONDARY,
				hoverText: 'Give gem',
			},
		},
	],
})
```

Players will see multiple labels, one for each pointer event, displayed radially around the cursor.

The example below combines using `PointerEvents` to show hover hints, together with a system that actually handles the player's action with custom logic.

```ts
// create entity
const myEntity = engine.addEntity()

// give the entity hover feedback
PointerEvents.create(myEntity, {
	pointerEvents: [
		{
			eventType: PointerEventType.PET_DOWN,
			eventInfo: {
				button: InputAction.IA_POINTER,
				hoverText: 'Open',
			},
		},
	],
})

// handle click events on the entity
engine.addSystem(() => {
	if (
		inputSystem.isTriggered(
			InputAction.IA_POINTER,
			PointerEventType.PET_DOWN,
			myEntity
		)
	) {
		// Custom logic in response to an input action
	}
})
```

### Hover Feedback

When a player hovers the cursor over an item with an `PointerEvents` component, they see:

* An edge highlight on the entity
* A hover hint near the cursor with an icon for the button they need to press and a string that reads "Interact".

These elements can be toggled and customized.

The hover feedback on the UI displays a different icon depending on what input you select in the `button` field. On PC, it displays an icon with an `E` for `InputAction.IA_PRIMARY`, an `F` for `InputAction.IA_SECONDARY`, and a mouse for `InputAction.IA_POINTER`.

Change the string by changing the `hoverText` value. Keep this string short, so that it's quick to read and isn't too intrusive on the screen.

```ts
// create entity
const chest = engine.addEntity()

// give entity a PointerEvents component
PointerEvents.create(chest, {
	pointerEvents: [
		{
			eventType: PointerEventType.PET_DOWN,
			eventInfo: {
				button: InputAction.IA_POINTER,
				hoverText: 'Open',
			},
		},
	],
})
```

In the example above, the pointer event includes a value for `hoverText`. This field defines the string to display in the UI while the player points at the entity. By default, this string spells _Interact_.


> [!NOTE]
> **💡 Tip**: The `hoverText` string should describe the action that happens when interacting. For example `Open`, `Activate`, `Grab`, `Select`. These strings should be as short as possible, to avoid stealing too much focus from the player.


If an entity has multiple pointer events on it, the hover hints for each of these are displayed radially around the cursor.

The `hoverText` of an `.UP` pointer event is only displayed while the player is already holding down the corresponding key and pointing at the entity.

If an entity has both a `DOWN` pointer event and an `UP` pointer event, the hint for the `DOWN` action is shown while the button is not being pressed. The hint switches to the one from the `UP` event only when the button is pressed and remains pressed.

```ts
// create entity
const entity = engine.addEntity()

// give entity a PointerEvents component
PointerEvents.create(entity, {
	pointerEvents: [
		{
			eventType: PointerEventType.PET_DOWN,
			eventInfo: {
				button: InputAction.IA_POINTER,
				hoverText: 'Drag',
			},
		},
		{
			eventType: PointerEventType.PET_UP,
			eventInfo: {
				button: InputAction.IA_POINTER,
				hoverText: 'Drop',
			},
		},
	],
})
```

To hide the hover hint, but leave the edge highlight, set the value of the `hoverText` to "".

```ts
// create entity
const chest = engine.addEntity()

// give entity a PointerEvents component
PointerEvents.create(chest, {
	pointerEvents: [
		{
			eventType: PointerEventType.PET_DOWN,
			eventInfo: {
				button: InputAction.IA_POINTER,
				hoverText: '',
			},
		},
	],
})
```

To hide the edge highlight but leave the hover hint, set `showHighlight` to _false_.

```ts
// create entity
const chest = engine.addEntity()

// give entity a PointerEvents component
PointerEvents.create(chest, {
	pointerEvents: [
		{
			eventType: PointerEventType.PET_DOWN,
			eventInfo: {
				button: InputAction.IA_POINTER,
				hoverText: 'Open door',
				showHighlight: false,
			},
		},
	],
})
```

To hide both the hover hint and the edge highlight, set `showFeedback` to _false_. When doing this, the cursor doesn't show any icons, text or any edge highlight. You could also just remove the `PointerEvents` component from the entity.

```ts
// create entity
const chest = engine.addEntity()

// give entity a PointerEvents component
PointerEvents.create(chest, {
	pointerEvents: [
		{
			eventType: PointerEventType.PET_DOWN,
			eventInfo: {
				button: InputAction.IA_POINTER,
				showFeedback: false,
			},
		},
	],
})
```

### Max distance

Some entities can be intentionally only interactive at a close range. If a player is too far away from an entity, the hover hint won't be displayed next to the cursor.

By default, entities are only clickable when the player's **avatar** is within _10 meters_ of the entity. You can change that range by setting the `maxDistance` property of a pointer event.

```ts
// create entity
const myEntity = engine.addEntity()

// give entity a PointerEvents component
PointerEvents.create(myEntity, {
	pointerEvents: [
		{
			eventType: PointerEventType.PET_DOWN,
			eventInfo: {
				button: InputAction.IA_POINTER,
				maxDistance: 6,
			},
		},
	],
})

// implement a system to handle the pointer event
engine.addSystem(() => {
	// fetch data about the pointer event
	const cmd = inputSystem.getInputCommand(
		InputAction.IA_POINTER,
		PointerEventType.PET_DOWN,
		myEntity
	)

	// check if the click was close enough
	if (cmd && cmd.hit && cmd.hit.length < 6) {
		// do something
	}
})
```

The example above sets the maximum distance for hover hints to _6 meters_. Make sure that the logic for handling the input actions also follows the same rules. See [Data from input action](/docs/creator/sdk7/interactivity/button-events/system-based-events/#data-from-input-action) for how to obtain the distance of an input action.

`maxDistance` is measured from the player's **avatar**, so it behaves the same in first and third person.

To limit interaction by how far the **active camera** is instead, use `maxCameraDistance`. Setting both means either check passing is enough. See [Distance limits](/docs/creator/sdk7/interactivity/button-events/register-callback/#distance-limits) for the full set of rules.


> [!WARNING]
> **📔 Note**: `maxPlayerDistance` is a deprecated alias of `maxDistance`. Both measure distance from the avatar, so use `maxDistance` in new scenes.


## Advanced custom hints

The `PointerEvents` component easily adds UI hints when the player's cursor starts hovering over an entity. It's generally a good thing that hints behave consistently with what players are used to seeing in other Decentraland scenes. However, in some cases you might want to signal that something is interactive in a custom way. For example, you could play a subtle sound when the player starts hovering over the entity. You could also show a glowing highlight around the entity while hovering, and hide it when no longer hovering. It could also be used for specific gameplay mechanics.

Use the `inputSystem.isTriggered()` function together with the `PointerEventType.PET_HOVER_ENTER` and `PointerEventType.PET_HOVER_LEAVE` events to carry out custom behaviors whenever the player's cursor starts pointing at the entity's collider, and whenever the cursor stops pointing at it.

The example below enlarges entities to a size of _1.5_ when the cursor starts pointing at their collider, and sets them back at a size of _1_ when the cursor leaves them.

```ts
engine.addSystem(() => {
	const meshEntities = engine.getEntitiesWith(MeshRenderer)
	for (const [entity] of meshEntities) {
		if (
			inputSystem.isTriggered(
				InputAction.IA_POINTER,
				PointerEventType.PET_HOVER_ENTER,
				entity
			)
		) {
			Transform.getMutable(entity).scale = Vector3.create(1.5, 1.5, 1.5)
		}

		if (
			inputSystem.isTriggered(
				InputAction.IA_POINTER,
				PointerEventType.PET_HOVER_LEAVE,
				entity
			)
		) {
			Transform.getMutable(entity).scale = Vector3.create(1, 1, 1)
		}
	}
})
```


> [!WARNING]
> **📔 Note**: Every entity you want to inteact with must have both a [`PointerEvents` component](/docs/creator/sdk7/interactivity/button-events/system-based-events/#show-feedback) and a [collider](/docs/creator/sdk7/3d-essentials/colliders/).


## Proximity events

Check for proximity button presses regardless of where their cursor is aiming by using `PET_DOWN` on an entity that uses `InteractionType.PROXIMITY`. Set the `interactionType` field in the pointer event definition, next to `eventInfo`, to mark the event as proximity-based.

See [**Proximity Events**](/docs/creator/sdk7/interactivity/button-events/proximity-events/) for more details, including the available helper functions.

```ts
PointerEvents.create(myEntity, {
	pointerEvents: [
		{
			eventType: PointerEventType.PET_DOWN,
			eventInfo: {
				button: InputAction.IA_PRIMARY,
				hoverText: 'Press E',
				maxDistance: 5
			},
			interactionType: InteractionType.PROXIMITY,
		},
	],
})

engine.addSystem(() => {
	if (inputSystem.isTriggered(InputAction.IA_PRIMARY, PointerEventType.PET_DOWN, myEntity)) {
		console.log('Player pressed button near entity')
	}
})
```

With helpers like `pointerEventsSystem.onProximityDown`, an entity can hold one cursor handler and one proximity handler at the same time, and both callbacks run. What it can't do is register two different buttons for the same handler. System-based events have neither limitation: you define one entry per behavior. For example here's an entity that can be interacted with both by pointing a cursor and pressing E, or by walking near it and pressing E without pointing at it with the cursor.

```ts
	PointerEvents.create(myEntity, {
        pointerEvents: [
            {
                eventType: PointerEventType.PET_DOWN,
                eventInfo: {
                    button: InputAction.IA_POINTER,
                    hoverText: 'Click',
                    maxDistance: 15,
                },
                interactionType: InteractionType.CURSOR,
            },
            {
                eventType: PointerEventType.PET_DOWN,
                eventInfo: {
                    button: InputAction.IA_PRIMARY,
                    hoverText: 'Press E',
                    maxDistance: 15,
                },
                interactionType: InteractionType.PROXIMITY,
            },
        ],
    })
    
    engine.addSystem(() => {
        if (inputSystem.isTriggered(InputAction.IA_POINTER, PointerEventType.PET_DOWN, myEntity)) {
            console.log('Player clicked at entity')
        }
        if (inputSystem.isTriggered(InputAction.IA_PRIMARY, PointerEventType.PET_DOWN, myEntity)) {
            console.log('Player pressed button near entity')
        }
    })
```


You can also use `inputSystem.isTriggered()` with the `PET_PROXIMITY_ENTER` and `PET_PROXIMITY_LEAVE` event types to react when a player enters or leaves the proximity range of an entity. This is useful to display extra feedback for proximity-activated entities, for example playing a sound or an animation.

For this to work, the entity must have a `PointerEvents` component with the corresponding proximity event type defined.

```ts
PointerEvents.create(myEntity, {
	pointerEvents: [
		{
			eventType: PointerEventType.PET_PROXIMITY_ENTER,
			eventInfo: {
				button: InputAction.IA_POINTER,
				maxDistance: 5,
			},
		},
		{
			eventType: PointerEventType.PET_PROXIMITY_LEAVE,
			eventInfo: {
				button: InputAction.IA_POINTER,
				maxDistance: 5,
				showFeedback: false,
			},
		},
	],
})

engine.addSystem(() => {
	if (inputSystem.isTriggered(InputAction.IA_POINTER, PointerEventType.PET_PROXIMITY_ENTER, myEntity)) {
		console.log('Player entered proximity')
	}
	if (inputSystem.isTriggered(InputAction.IA_POINTER, PointerEventType.PET_PROXIMITY_LEAVE, myEntity)) {
		console.log('Player left proximity')
	}
})
```


## Data from input action

Fetch data from an input action, such as the button that was pressed, the entity that was hit, the direction and length of the ray, etc. See ([See documentation](/docs/)) for a description of all of the data available.

To fetch this data, use `inputSystem.getInputCommand`. This function returns the full data structure with data about the input event.

```ts
engine.addSystem(() => {
	const cmd = inputSystem.getInputCommand(
		InputAction.IA_POINTER,
		PointerEventType.PET_DOWN,
		myEntity
	)
	if (cmd) {
		console.log(cmd.hit?.entityId)
	}
})
```

If there was no input action that matches the query, then `inputSystem.getInputCommand` returns _null_. Make sure that you handle this scenario in your logic.

### Max click distance

To enforce a maximum distance, so that an entity is only clickable at close range, fetch `hit.length` property of the event data.

```ts
// implement a system to handle the pointer event
engine.addSystem(() => {
	// fetch data about the pointer event
	const cmd = inputSystem.getInputCommand(
		InputAction.IA_POINTER,
		PointerEventType.PET_DOWN,
		myEntity
	)

	// check if the click was close enough
	if (cmd && cmd.hit && cmd.hit.length < 6) {
		// do something
	}
})
```


> [!WARNING]
> **📔 Note**: If you ignore any events that are far away, make sure you set the `maxDistance` parameter on the `PointerEvents` component to behave consistently.


### Different meshes inside a model

Often, _.glTF_ 3D models are made up of multiple meshes, that each have an individual internal name. All button events events include the information of what specific mesh was clicked, so you can use this information to trigger different click behaviors in each case.

To see how the meshes inside the model are named, you must open the 3D model with an editing tool, like [Blender](https://www.blender.org/) for example.

![](</docs/.gitbook/assets/mesh-names (1).webp>)


> [!NOTE]
> **💡 Tip**: You can also learn the name of the clicked mesh by logging it and reading it off console.


You access the `meshName` property as part of the `hit` object, that's returned by the click event.

In the example below we have a house model that includes a mesh named `firePlace`. We want to turn on the fireplace only when its corresponding mesh is clicked.

```ts
engine.addSystem(() => {
	const cmd = inputSystem.getInputCommand(
		InputAction.IA_POINTER,
		PointerEventType.PET_DOWN,
		myEntity
	)
	if (cmd && cmd.hit?.meshName === 'firePlace') {
		// light fire
	}
})
```


> [!WARNING]
> **📔 Note**: This only applies to pointer-based events, not to proximity-based events, since in that case all of the meshes of the model could be in proximity of the player at once.

