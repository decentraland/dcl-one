> Learn how to handle user clicks in your scene.

# About input actions

A Decentraland scene can detect input actions from all of the buttons that are used to control the player's avatar. These include pointer clicks, several action buttons, and the keys that are used to move the avatar around. Button events can come from a mouse and keyboard on the desktop client, from the on-screen controls on the [mobile app](/docs/creator/build-for-mobile/mobile-client/overview/), from a VR controller, or from other input devices — these are all interpreted the same by the SDK.

You can detect input actions against an entity. This involves pressing a button while the player's cursor is pointing at that entity's collider. You can also detect _global_ input event, that involve pressing activating the input at any time, without consideration for where the pointer is aiming.


> [!WARNING]
> **📔 Note**: Entities must have a [collider](/docs/creator/sdk7/3d-essentials/colliders/) to respond to input actions. `MeshRenderer` models must also be given a `MeshCollider` component. Models from a `GltfContainer` may have their own embedded collision geometry, or they can be configured to use their visible geometry, they can also be given a `MeshCollider` component.


There are several different ways to handle input actions, depending on the use case.

* [**Register a callback**](/docs/creator/sdk7/interactivity/button-events/register-callback/): The easiest way to add interaction to a single entity. Write a single statement to set up a callback function and hover feedback.
* [**System-based**](/docs/creator/sdk7/interactivity/button-events/system-based-events/): Ideal for handling multiple entities with similar behavior. Use a system to iterate over similar entities and query for input actions on each, handling them all with the same logic. Hover feedback needs to be set up separately. This approach is also required for handling global input actions.
* [**Advanced**](/docs/creator/sdk7/interactivity/button-events/advanced-button-events/): Read the raw response data on each entity, including time-stamps and an event history of input events. This can be useful for defining custom interaction patterns.

## Use the Scene Editor in Creator Hub

The easiest way to handle click events on an entity is to use the Scene Editor. Use the no-code **On Click** or **On Input Action** Triggers on an item to call actions when clicking on it. Or use **On Global Click**, **On Global Primary** or **On Global Secondary** Triggers to react to global button events. See [Make any item smart](/docs/creator/scene-editor/interactivity/make-any-item-smart/).

## Simple example

To detect clicks on an entity, use `pointerEventsSystem.onPointerDown`.

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

See [**Register a callback**](/docs/creator/sdk7/interactivity/button-events/register-callback/) for more information.

## Hover Feedback

It's important to make players aware that an entity is interactive. Otherwise, they might completely miss out on the experience you built. It's not a good experience to be clicking on every object hoping for one to respond.

When you use the [**Register a callback**](/docs/creator/sdk7/interactivity/button-events/register-callback/) method, two kinds of feedback are displayed whenever the player passes their cursor over the object:

* The entity's edge is highlighted (only on the Decentraland 2.0 Desktop client). The highlight is green if the entity is close enough to click, red if the entity is too far away.
* A hover hint appears near the cursor with UI text, signalling what will happen if they click.

When using the [**System-based**](/docs/creator/sdk7/interactivity/button-events/system-based-events/) method, you can achieve the same results by adding a `PointerEvents` component to the clickable entities.

Both the entity highlight and the hover hint can be disabled via properties in these methods and components.

You could also implement [advanced custom hints](/docs/creator/sdk7/interactivity/button-events/system-based-events/#advanced-custom-hints), for example you could play a sound, making the entity change color, spin or enlarge while being pointed at, etc. Whatever you do, make sure that it's a clear signifier.

## Obstacles

Button events cast rays that only interact with the first entity on their path that is subscribed to the pointer events collision layer. This is true as long as the entity is within its distance limits. If the entity defines distance limits for both the camera and the avatar, it's enough for either one of the two to be close enough.

For an entity to be intercepted by the ray of a pointer event, either:

* The model must contain [collider meshes](/docs/creator/3d-modeling/colliders/).
* The `GltfContainer` must be configured to use the [visible geometry with collision masks](/docs/creator/sdk7/3d-essentials/colliders/#colliders-on-3d-models).
* The entity must have a [MeshCollider component](/docs/creator/sdk7/3d-essentials/colliders/).

If another entity's collider is standing on the way of the entity that the player wants to interact with it, the player won't be able to click the entity that's behind, unless the entity has no collider, or this collider is configured to not respond to the pointer events collision layer.

```ts
// clickable entity
const clickableEntity = engine.addEntity()
MeshRenderer.setBox(clickableEntity)
MeshCollider.setBox(clickableEntity)
Transform.create(clickableEntity, { position: Vector3.create(8, 1, 8) })

pointerEventsSystem.onPointerDown(
	{
		entity: clickableEntity,
		opts: {
			button: InputAction.IA_POINTER,
			hoverText: 'Click',
		},
	},
	function () {
		console.log('clicked entity')
		const t = Transform.getMutable(clickableEntity)
		t.scale.y += 0.2
	}
)

// non-blocker for clicks
const nonBlocker = engine.addEntity()
MeshRenderer.setBox(nonBlocker)
MeshCollider.setBox(nonBlocker, ColliderLayer.CL_PHYSICS)
Transform.create(nonBlocker, { position: Vector3.create(10, 1, 8) })

// blocker for clicks
const blocker = engine.addEntity()
MeshRenderer.setBox(blocker)
MeshCollider.setBox(blocker, ColliderLayer.CL_POINTER)
Transform.create(blocker, { position: Vector3.create(8, 1, 10) })
```


> [!WARNING]
> **📔 Note**: For an entity to not only intercept a pointer event, but also to return data, the entity also needs to have a `PointerEvents` component. The `pointerEventsSystem` helpers also take care of this requirment.


## Pointer buttons

The following inputs can be handled by any of the approaches to detect input events.

* `InputAction.IA_POINTER`: **left-mouse button** on a computer, **interaction button** on mobile.
* `InputAction.IA_PRIMARY`: **E** key on a computer, **E button** on mobile.
* `InputAction.IA_SECONDARY`: **F** key on a computer, **F button** on mobile.
* `InputAction.IA_ACTION_3`: **1** key on a computer, **1 button** on mobile. _Not easily reachable on mobile — not recommended for mobile interaction._
* `InputAction.IA_ACTION_4`: **2** key on a computer, **2 button** on mobile. _Not easily reachable on mobile — not recommended for mobile interaction._
* `InputAction.IA_ACTION_5`: **3** key on a computer, **3 button** on mobile. _Not easily reachable on mobile — not recommended for mobile interaction._
* `InputAction.IA_ACTION_6`: **4** key on a computer, **4 button** on mobile. _Not easily reachable on mobile — not recommended for mobile interaction._
* `InputAction.IA_JUMP`: **Space** key on a computer, **jump button** on mobile.
* `InputAction.IA_FORWARD`: **W** key on a computer.
* `InputAction.IA_LEFT`: **A** key on a computer.
* `InputAction.IA_RIGHT`: **D** key on a computer.
* `InputAction.IA_BACKWARD`: **S** key on a computer.
* `InputAction.IA_WALK`: **Control** key on a computer.
* `InputAction.IA_MODIFIER`: **Shift** key on a computer.

Each `InputAction` is abstracted away from the literal input in the keyboard so that it can be mapped to different inputs depending on the device. For this same reason, not all buttons on the keyboard can be tracked for button events, only the buttons that are used for movement and interaction. This intentional limitation ensures that content is compatible across desktop, the [mobile client](/docs/creator/build-for-mobile/mobile-client/overview/), and future devices like VR controllers and other game controllers.


> [!WARNING]
> **📱 Mobile**: On the [mobile client](/docs/creator/build-for-mobile/mobile-client/overview/), all input actions are available, but `IA_ACTION_3`–`IA_ACTION_6` (the `1`/`2`/`3`/`4` buttons) are tucked away behind a secondary menu and are not easily reachable during gameplay. When designing for mobile, prefer `IA_POINTER` (interaction button), `IA_PRIMARY` (E button), and `IA_SECONDARY` (F button) for your key actions. See [Input on mobile](/docs/creator/build-for-mobile/develop/input-on-mobile/).


## Types of pointer events

Each input can produce the following types of pointer events. Each of the following is a value in the `PointerEventType` enum.

* `PET_DOWN`: Player pushes down a specific button while having the cursor pointing at the entity's collider.
* `PET_UP`: Player releases a specific button while having the cursor pointing at the entity's collider.
* `PET_HOVER_ENTER`: Player's cursor starts pointing at the entity's collider.
* `PET_HOVER_LEAVE`: Player's cursor stops pointing at the entity's collider.
* `PET_PROXIMITY_ENTER`: Player walks within the entity's proximity range, regardless of where they're looking.
* `PET_PROXIMITY_LEAVE`: Player moves out of the entity's proximity range.

See [**Proximity Events**](/docs/creator/sdk7/interactivity/button-events/proximity-events/) for how to use proximity-based interactions.

## Data from an input action

All input actions include data about the event, including things like the button that was activated, and where the pointer was pointing at at the time.

The following information can be obtained from any input event:

* `analog`: Optional number, only present on events from an analog input (like a joystick), storing the input's analog value.
* `button`: Which button id was pressed. The number corresponds to the `InputAction` enum, that lists all of the available buttons.
* `state`: Type of pointer event, from the enum `PointerEventType`. _0_ refers to `PointerEventType.PET_UP`, _1_ to `PointerEventType.PET_DOWN`, _2_ to `PointerEventType.PET_HOVER_ENTER`, _3_ to `PointerEventType.PET_HOVER_LEAVE`, _4_ to `PointerEventType.PET_PROXIMITY_ENTER`, _5_ to `PointerEventType.PET_PROXIMITY_LEAVE`

* `timestamp`: A [lamport timestamp](https://en.wikipedia.org/wiki/Lamport_timestamp) to identify each button event.

  > Note: This timestamp is not numbered based on the current time. Think of it as a counter that starts at 0 and is incremented by 1 for each event.

* `hit`: An object that contains the following data about the hit event:

  * `entityId`: Id number of the entity that was hit by the ray.
  * `meshName`: _String_ with the internal name of the specific mesh in the 3D model that was hit. This is useful when a 3D model is composed of multiple meshes.
  * `globalOrigin`: _Vector3_ for the position where the ray originates (relative to the scene)
  * `direction`: _Vector3_ with the direction vector of the ray, in global coordinates
  * `position`: _Vector3_ for the position where the ray intersected with the hit entity (relative to the scene)
  * `length`: Length of the ray from its origin to the position where the hit against the entity occurred.
  * `normalHit`: _Vector3_ with a normalized direction vector, describing the angle of the normal of the hit in world space.

This data is accessed in different ways depending on what approach you're using to handle input actions.

Using the [**Register a callback**](/docs/creator/sdk7/interactivity/button-events/register-callback/) approach, the first parameter passed to the callback function contains this entire data structure.

```ts
pointerEventsSystem.onPointerDown({ entity: myEntity }, function (cmd) {
	console.log(cmd.hit?.entityId)
})
```

Using the [**System-based**](/docs/creator/sdk7/interactivity/button-events/system-based-events/) approach, use `inputSystem.getInputCommand()` to fetch this data.

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


> [!WARNING]
> **📔 Note**: For an entity to not only intercept a pointer event, but also to return data, the entity also needs to have a `PointerEvents` component. The `pointerEventsSystem` helpers also take care of this requirment.


Using the [**Advanced**](/docs/creator/sdk7/interactivity/button-events/advanced-button-events/) approach, the `PointerEventsResult` contains a list with a recent history of all pointer events against that entity.

```ts
engine.addSystem(() => {
	for (const [entity] of engine.getEntitiesWith(PointerEventsResult)) {
		const pointerEvents = PointerEventsResult.get(entity)

		for (const event of pointerEvents) {
			console.log(event.hit?.entityId)
		}
	}
})
```

## Lock or unlock the cursor


> [!NOTE]
> **💡 Note**: `PointerLock` is a desktop-client concept (locked vs. unlocked mouse cursor). It has no effect on the [mobile app](/docs/creator/build-for-mobile/mobile-client/overview/), where input is touch-based.


Players in Decentraland's desktop client can toggle between two distinct interaction modes with their cursor:

* Locked cursor: moving the mouse shifts the camera and click events occur in the crosshair at the center of the screen 
* Unlocked cursor: the cursor is free to move independently of the camera, and players can click anywhere on the screen to interact with either the 3D space or the UI

These modes of interaction are ideal for different kinds of game mechanics, and you may want to make players adopt one or the other depending on what suits your scene best. You may also want to momentarily ensure the player's cursor is unlocked to allow them to interact with a UI, this is especially helpful for new players that aren't familiar with how this is controlled.

To change the player's cursor state, use the `PointerLock` component on the `engine.CameraEntity` entity.

```ts
import {PointerLock} from '@dcl/sdk/ecs'
    
PointerLock.createOrReplace(engine.CameraEntity, {isPointerLocked: false})
```


> [!WARNING]
> **📔 Note**: The engine provides a `PointerLock` component on the `engine.CameraEntity` by default. Use `createOrReplace()` or `getMutable()` rather than `create()`, which throws an error if the component already exists.


You can also query the player's cursor state by reading the `PointerLock` component's state.

```ts
import {PointerLock} from '@dcl/sdk/ecs'

const isPointerLocked = PointerLock.getOrNull(engine.CameraEntity)?.isPointerLocked
```

Use the `.onChange` function to react in changes in the pointer state. The following example enforces that the cursor is always unlocked.

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
