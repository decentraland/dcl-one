> Learn how to handle user clicks in your scene.

# Register Callback

The easiest way to handle button events is to register a callback function for a particular entity. Every time that entity is interacted with using a specific button, the callback function is called.

If you need to add the same behavior to multiple similar entities, consider using the [System-based](/docs/creator/sdk7/interactivity/button-events/system-based-events/) approach instead of adding callbacks to each entity. The system-based approach can result in more efficiency as you iterate over a list of similar entities.

The Register callback approach is especially useful if you want to describe a behavior that affects a single entity, as it's more straight forward.


> [!WARNING]
> **📔 Note**:\
> For an entity to be interactive, it **must** have a [collider](/docs/creator/sdk7/3d-essentials/colliders/). See [obstacles](/docs/creator/sdk7/interactivity/button-events/click-events/#obstacles) for more details.


## Pointer down

Use `pointerEventsSystem.onPointerDown()` to detect presses of a specific button.

This statement requires two parameters:

* `data`: An object that contains the following:
  * `entity`: The entity to handle
  * `opts`: An object with optional additional data:
    * `button`: Which button to listen for. See [Pointer buttons](/docs/creator/sdk7/interactivity/button-events/click-events/#pointer-buttons) for supported options. If no button is specified, then all buttons are listened to, including movement buttons like forward and jump.
    * `maxDistance`: The maximum distance between the entity and the player's **avatar**, in meters. 10 by default. See [Distance limits](#distance-limits).
	* `maxCameraDistance`: The maximum distance between the entity and the **active camera**, in meters. Not set by default. See [Distance limits](#distance-limits).
    * `hoverText`: What string to display in the hover feedback hint. "Interact" by default.
    * `showFeedback`: If false, it hides both the hover hint and the edge highlight for this entity. _true_ by default.
    * `showHighlight`: If true, players will see the edge highlight when hovering the cursor on the entity. _true_ by default. This value is only considered if `showFeedback` is _true_.
* `cb`: A callback function to run each time a button down event occurs while pointing at the entity

```ts
pointerEventsSystem.onPointerDown(
	{
		entity: myEntity,
		opts: { button: InputAction.IA_PRIMARY, hoverText: 'Click' },
	},
	function () {
		console.log('clicked entity')
	}
)
```

The above command leaves the callback function registered, and it will be called every time the related button event occurs. Note that the callback must be a synchronous function; `async` functions aren't supported here and result in an error.


> [!WARNING]
> **📔 Note**:\
> Only one `pointerEventsSystem.onPointerDown` can be registered per entity. Once added, it will keep listening for events till the listener is removed. Do not run this recurrently within a system, as that would keep rewriting the pointer event behavior.


## Hover Feedback

It's very important to give players some kind of indication that an entity can be interacted with.

When registering an input action with the `EventsSystem`, by default players will see:

* An edge highlight on the entity
* A hover hint near the cursor with an icon for the button they need to press and a string that reads "Interact".

These elements can be toggled and customized.

The hover feedback on the UI displays a different icon depending on what input you select in the `button` field. On PC, it displays an icon with an `E` for `InputAction.IA_PRIMARY`, an `F` for `InputAction.IA_SECONDARY`, and a mouse for `InputAction.IA_POINTER`.

Change the string by changing the `hoverText` value. Keep this string short, so that it's quick to read and isn't too intrusive on the screen.

```ts
pointerEventsSystem.onPointerDown(
	{
		entity: myEntity,
		opts: { button: InputAction.IA_PRIMARY, hoverText: 'Open door' },
	},
	function () {
		// open door
	}
)
```

To hide the hover hint, but leave the edge highlight, set the value of the `hoverText` to "".

```ts
pointerEventsSystem.onPointerDown(
  {entity: myEntity, opts: { button: InputAction.IA_PRIMARY, hoverText: ''}},
  function () {
    console.log("clicked on surprise interactive item")
  }
)
```

To hide the edge highlight but leave the hover hint, set `showHighlight` to _false_.

```ts
pointerEventsSystem.onPointerDown(
	{
		entity: myEntity,
		opts: {
			button: InputAction.IA_PRIMARY,
			hoverText: 'Open door',
			showHighlight: false,
		},
	},
	function () {
		console.log('opened secret door')
	}
)
```

To hide both the hover hint and the edge highlight, set `showFeedback` to _false_. When doing this, the cursor doesn't show any icons, text or any edge highlight.

```ts
pointerEventsSystem.onPointerDown(
  {entity: myEntity, opts: { button: InputAction.IA_PRIMARY, showFeedback: false}},
  function () {
    console.log("opened secret door")
  }
)
```

### Change existing feedback

When registering an input action with the `EventsSystem`, this is creating a `PointerEvents` component and adding it to the interactive entity behind the scenes. This component handles the behavior of the UI hover hint. To change the behavior of the hover feedback, modify this component. See [Show feedback](/docs/creator/sdk7/interactivity/button-events/system-based-events/#show-feedback) for more about how to deal with this component.

```ts
const hoverFeedback = PointerEvents.getMutable(myEntity)

if (hoverFeedback.pointerEvents[0]?.eventInfo) {
	hoverFeedback.pointerEvents[0].eventInfo.hoverText = 'Close door'
}
```

## Distance limits

By default, a player can only interact with an entity when their **avatar** is within 10 meters of it. If the player is too far away, the entity's highlight effect turns red rather than green, and pointer events don't fire.

Use `maxDistance` to change that range:

```ts
// Only clickable when the player's avatar is within 5 meters
pointerEventsSystem.onPointerDown(
	{ entity: myEntity, opts: { maxDistance: 5 } },
	function () {
		console.log('clicked entity')
	}
)
```

### Limit by camera distance

`maxCameraDistance` measures from the **currently active camera** instead of the avatar. This is useful when your scene uses a [Virtual Camera](/docs/creator/sdk7/3d-essentials/camera/#using-virtual-cameras) placed away from the player, and you want the entity to be clickable based on what the camera can see.

```ts
// Only clickable when the active camera is within 15 meters
pointerEventsSystem.onPointerDown(
	{ entity: myEntity, opts: { maxCameraDistance: 15 } },
	function () {
		console.log('clicked entity')
	}
)
```

### How the two limits combine

The two options are independent checks. Which ones run depends on which you set:

| What you set | What the player needs |
| --- | --- |
| Neither | Avatar within 10 meters (the default) |
| `maxDistance` only | Avatar within `maxDistance` |
| `maxCameraDistance` only | Camera within `maxCameraDistance` |
| Both | **Either** check passing is enough |

Setting both is a way to say "close enough to walk up to it, **or** close enough to see it clearly":

```ts
// Clickable when the avatar is within 3 meters OR the camera is within 20 meters
pointerEventsSystem.onPointerDown(
	{ entity: myEntity, opts: { maxDistance: 3, maxCameraDistance: 20 } },
	function () {
		console.log('clicked entity')
	}
)
```


> [!WARNING]
> **📔 Note**: `maxCameraDistance` requires `@dcl/sdk` version 7.28.0 or newer. Support for it is already available in the Bevy-based explorer. If your scene must behave identically on every client today, rely on `maxDistance` and treat `maxCameraDistance` as an enhancement.


### maxPlayerDistance is deprecated

`maxPlayerDistance` is a deprecated alias of `maxDistance`: both measure distance from the avatar. Use `maxDistance` in new scenes.

If a scene sets both, the **larger** of the two is used as the avatar distance limit.

## Pointer up

Use `pointerEventsSystem.onPointerUp` to register a callback function that gets called when the indicated player lets the button up while pointing at the entity.

```ts
pointerEventsSystem.onPointerUp(
	{
		entity: myEntity,
		opts: { button: InputAction.IA_PRIMARY, hoverText: 'Button up' },
	},
	function () {
		console.log('button up')
	}
)
```

This statement requires two parameters:

* `data`: An object that contains the following:
  * `entity`: The entity to handle
  * `opts`: An object with optional additional data:
    * `button`: Which button to listen for. See [Pointer buttons](/docs/creator/sdk7/interactivity/button-events/click-events/#pointer-buttons) for supported options. If no button is specified, then all buttons are listened to, including movement buttons like forward and jump.
    * `hoverText`: What string to display in the hover feedback hint. "Interact" by default.
    * `showFeedback`: If false, it hides the hover hint for this entity. _true_ by default.
    * `maxDistance`: How far the player's avatar can be from the entity and still interact with it, in meters. 10 by default. If the player is too far, there will be no hover feedback and pointer events won't work. See [Distance limits](#distance-limits).
* `cb`: A callback function to run each time a button up event occurs while pointing at the entity.

A same entity can have two different callbacks registered, one for `pointerEventsSystem.onPointerDown` and one for `pointerEventsSystem.onPointerUp`. The entity can only register one callback of each, [Handle multiple buttons](/docs/creator/sdk7/interactivity/button-events/register-callback/#handle-multiple-buttons) to detect different buttons on one same callback.


> [!WARNING]
> **📔 Note**: The hover feedback for a button up event is only displayed when the button is currently pushed down. If the player points at the entity without holding the button down, they will see no feedback, or the feedback for the button down event, if any.


## Hover enter and leave

Use `pointerEventsSystem.onPointerHoverEnter` to run a callback when the player's cursor starts pointing at an entity, and `pointerEventsSystem.onPointerHoverLeave` when the cursor stops pointing at it.

```ts
pointerEventsSystem.onPointerHoverEnter(
    {
        entity: myEntity,
        opts: { button: InputAction.IA_POINTER },
    },
    function () {
        console.log('Cursor started hovering over entity')
    }
)

pointerEventsSystem.onPointerHoverLeave(
    {
        entity: myEntity,
        opts: { button: InputAction.IA_POINTER },
    },
    function () {
        console.log('Cursor stopped hovering over entity')
    }
)
```

These callbacks are useful for custom hover effects, such as playing a sound or animating the entity when the player aims at it.

## Remove callbacks

To remove a callback function, use the corresponding remove function:

```ts
pointerEventsSystem.removeOnPointerDown(myEntity)
pointerEventsSystem.removeOnPointerUp(myEntity)
pointerEventsSystem.removeOnPointerHoverEnter(myEntity)
pointerEventsSystem.removeOnPointerHoverLeave(myEntity)
```

Once removed, the hover feedback on the entity should no longer be displayed, and the entity should no longer be interactive.

Each remove function takes away only the entry that its matching register function added. Registering and removing the same callback repeatedly does not leave stale entries behind on the entity.

## Proximity interactions

For interactions that trigger based on player proximity rather than cursor aim, see [**Proximity Events**](/docs/creator/sdk7/interactivity/button-events/proximity-events/). The `pointerEventsSystem` includes equivalent helpers (`onProximityDown`, `onProximityUp`, `onProximityEnter`, and `onProximityLeave`) that follow the same pattern as the functions described on this page.

An entity can carry both a cursor handler and a proximity handler for the same event type. Both callbacks run, each when its own condition is met.

### Data from input action

Fetch data from an input action, such as the button that was pressed, the entity that was hit, the direction and length of the ray, etc. See ([See documentation](/docs/)) for a description of all of the data available.

To fetch this data, pass a parameter to the callback function. This parameter contains the full data structure with data about the input event.

```ts
pointerEventsSystem.onPointerDown(
	{ entity: myEntity, opts: { button: InputAction.IA_PRIMARY } },
	function (cmd) {
		console.log(cmd.hit?.entityId)
	}
)
```

### Handle multiple buttons

You can't register more than one `onPointerDown` on a single entity. Ideally you should use the [System-based](/docs/creator/sdk7/interactivity/button-events/system-based-events/) approach, as this allows you to handle as many different inputs as you wish, and display a UI hover feedback hint for each button.

As an alternative, you can use the Register callback approach and set the `button` field as `InputAction.IA_ANY`.

```ts
pointerEventsSystem.onPointerDown(
  {entity: myEntity, opts: { button: InputAction.IA_ANY}},
  function (cmd) {
      if(cmd.button === InputAction.IA_POINTER){
        // do X
      } else if (cmd.button === InputAction.IA_PRIMARY){
        // do Y
      }
  }
)
```

This approach is not ideal, as the hover hint shows a single string and won't specify what action to activate. Note that this will make the callback function run for every input action including movement keys, so you must filter out only the actions you care about.
