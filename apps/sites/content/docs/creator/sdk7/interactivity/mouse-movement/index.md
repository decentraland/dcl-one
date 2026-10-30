> React to the player's mouse movement in real time, to drive drag gestures, spin objects, or steer custom cameras.

# Mouse Movement

Your scene can read the player's raw mouse movement on every frame, and use it to drive real-time interactions: drag and swipe gestures, spinning or sliding objects as the player drags them, or steering a custom camera like in a first-person shooter.

To do this, read the `screenDelta` property of the `PrimaryPointerInfo` component on `engine.RootEntity`. This property is a `Vector2` that reports how many pixels the mouse moved since the last frame. Positive `x` values mean the mouse moved right, positive `y` values mean it moved up, matching the screen's origin at the bottom-left corner. On frames where the mouse doesn't move, both values are 0.

```ts
import { engine, PrimaryPointerInfo } from '@dcl/sdk/ecs'

engine.addSystem(() => {
	const delta = PrimaryPointerInfo.getOrNull(engine.RootEntity)?.screenDelta
	if (!delta) return
	console.log(`mouse moved: ${delta.x}, ${delta.y}`)
})
```

Since `screenDelta` only holds the movement of a single frame, always read it inside a [system](/docs/creator/sdk7/architecture/systems/), so no movement goes unnoticed between frames.


> [!WARNING]
> **📔 Note**: Avoid referring to the `engine.RootEntity` on the initial scene loading, because that can result in errors if the entities are not initialized yet. To avoid this problem, always refer to the entity inside a system. It will always be available, because the first execution of the system is called once the scene is already properly initialized.



> [!WARNING]
> **📔 Note**: What's described in this document is only relevant to players on desktop. On the [mobile app](/docs/creator/build-for-mobile/mobile-client/overview/), input is touch-based and there is no free-moving cursor, so `screenDelta` always reports 0. See [Input on mobile](/docs/creator/build-for-mobile/develop/input-on-mobile/) for how input works on touch devices, and [Detect platform](/docs/creator/build-for-mobile/develop/detect-platform/) to offer alternative controls to mobile players.


## Mouse movement while the cursor is locked

`screenDelta` behaves differently from the other properties of `PrimaryPointerInfo` when the cursor is [locked](/docs/creator/sdk7/interactivity/button-events/click-events/#lock-or-unlock-the-cursor): `screenCoordinates` freezes at the screen center and `worldRayDirection` always reports the ray at the center of the screen, but `screenDelta` keeps reporting the raw mouse movement on every frame. This makes it the only way to read mouse movement while the cursor is locked, which is exactly what you need for custom camera or aiming controls.

## Drag to rotate an object

The following example lets the player spin a cube by dragging the mouse sideways while holding the pointer button down:

```ts
import {
	engine,
	InputAction,
	inputSystem,
	MeshCollider,
	MeshRenderer,
	PrimaryPointerInfo,
	Transform,
} from '@dcl/sdk/ecs'
import { Quaternion, Vector3 } from '@dcl/sdk/math'

const DRAG_SENSITIVITY = 0.5

export function main() {
	const cube = engine.addEntity()
	Transform.create(cube, { position: Vector3.create(8, 1, 8) })
	MeshRenderer.setBox(cube)
	MeshCollider.setBox(cube)

	engine.addSystem(() => {
		// only rotate while the pointer button is held down
		if (!inputSystem.isPressed(InputAction.IA_POINTER)) return

		const delta = PrimaryPointerInfo.getOrNull(engine.RootEntity)?.screenDelta
		if (!delta || delta.x === 0) return

		const transform = Transform.getMutable(cube)
		transform.rotation = Quaternion.multiply(
			transform.rotation,
			Quaternion.fromEulerDegrees(0, delta.x * DRAG_SENSITIVITY, 0)
		)
	})
}
```

The same pattern works for any drag interaction: slide an entity along a rail using `delta.x` as an offset, scrub through an animation, or detect a quick swipe by checking for large delta values.

## Mouselook camera controls

Because `screenDelta` keeps working while the cursor is locked, you can use it to build mouselook controls for a [virtual camera](/docs/creator/sdk7/3d-essentials/camera/#using-virtual-cameras). To do this, accumulate the mouse movement into yaw and pitch angles, then apply them to the camera's rotation on every frame.

In the following example, the player clicks a box to switch to a virtual camera that they can steer with the mouse, and presses the secondary button (_F_ or right-click) to return to the default camera. While the virtual camera is active, the scene also locks the cursor and freezes the avatar with an [input modifier](/docs/creator/sdk7/interactivity/avatars/locomotion/#freeze-the-player), so the player doesn't walk around blindly while steering the camera.

```ts
import {
	engine,
	Entity,
	InputAction,
	InputModifier,
	inputSystem,
	MainCamera,
	MeshCollider,
	MeshRenderer,
	PointerEventType,
	pointerEventsSystem,
	PointerLock,
	PrimaryPointerInfo,
	Transform,
	VirtualCamera,
} from '@dcl/sdk/ecs'
import { Quaternion, Vector3 } from '@dcl/sdk/math'

// degrees of camera rotation per pixel of mouse movement
const SENSITIVITY = 0.15

let cameraEntity: Entity
let cameraActive = false
let yaw = 0
let pitch = 0

export function main() {
	// the entity that the camera follows
	cameraEntity = engine.addEntity()
	Transform.create(cameraEntity, { position: Vector3.create(8, 3, 8) })
	VirtualCamera.create(cameraEntity, {
		defaultTransition: { transitionMode: VirtualCamera.Transition.Time(0.5) },
	})

	// click this box to enter mouselook mode
	const box = engine.addEntity()
	Transform.create(box, { position: Vector3.create(8, 1, 4) })
	MeshRenderer.setBox(box)
	MeshCollider.setBox(box)
	pointerEventsSystem.onPointerDown(
		{
			entity: box,
			opts: { button: InputAction.IA_POINTER, hoverText: 'Control camera' },
		},
		() => activateCamera(true)
	)

	// steer the camera with the mouse
	engine.addSystem(() => {
		if (!cameraActive) return
		if (!PointerLock.getOrNull(engine.CameraEntity)?.isPointerLocked) return

		const delta = PrimaryPointerInfo.getOrNull(engine.RootEntity)?.screenDelta
		if (!delta) return

		yaw += delta.x * SENSITIVITY
		// clamp pitch so the camera can't flip over
		pitch = Math.max(-85, Math.min(85, pitch - delta.y * SENSITIVITY))

		Transform.getMutable(cameraEntity).rotation = Quaternion.fromEulerDegrees(pitch, yaw, 0)
	})

	// exit mouselook mode with the secondary button
	engine.addSystem(() => {
		if (!cameraActive) return
		if (inputSystem.isTriggered(InputAction.IA_SECONDARY, PointerEventType.PET_DOWN)) {
			activateCamera(false)
		}
	})
}

function activateCamera(active: boolean) {
	cameraActive = active

	// assign or release the virtual camera
	MainCamera.createOrReplace(engine.CameraEntity, {
		virtualCameraEntity: active ? cameraEntity : undefined,
	})

	// freeze the avatar while the camera is being steered
	InputModifier.createOrReplace(engine.PlayerEntity, {
		mode: InputModifier.Mode.Standard({ disableAll: active }),
	})

	// lock the cursor, so the mouse steers the camera right away
	PointerLock.createOrReplace(engine.CameraEntity, { isPointerLocked: active })
}
```

A few things to note about this example:

- The system subtracts `delta.y` from the pitch, so that moving the mouse up tilts the camera up. Flip that sign if you prefer inverted vertical controls.
- The pitch is clamped to a range of -85 to 85 degrees, so the camera can never flip over backwards.
- The `SENSITIVITY` constant expresses degrees of rotation per pixel of mouse movement, tweak it to taste.


> [!NOTE]
> **💡 Tip**: The player can unlock the cursor at any time by pressing _Esc_ or right-clicking, which stops the camera from responding to the mouse. Always give players a clear way to fully exit the camera mode, like the secondary button in the example above. You can also re-lock the cursor from code, see [Lock or unlock the cursor](/docs/creator/sdk7/interactivity/button-events/click-events/#lock-or-unlock-the-cursor).


## Related topics

To read the cursor's absolute position on the screen instead of its movement, use the `screenCoordinates` property of the same component. See [Check the player's cursor position](/docs/creator/sdk7/interactivity/user-data/#check-the-players-cursor-position).

To find out which entity is under the cursor, combine the `worldRayDirection` property with a raycast. See [Raycasting](/docs/creator/sdk7/interactivity/raycasting/).


> [!NOTE]
> **💡 Tip**: For working examples of pointer state, see the [`0,5-primary-cursor-info`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/0,5-primary-cursor-info) test scene, which reads `PrimaryPointerInfo` each frame and feeds `worldRayDirection` into a raycast; [`31,20-pointer-lock-control`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/31,20-pointer-lock-control), which requests and releases cursor capture by writing `PointerLock.isPointerLocked`; and [`32,20-virtual-camera-mouse-look`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/32,20-virtual-camera-mouse-look), which shows `screenDelta` still reporting movement while `screenCoordinates` is frozen at the screen center during lock.

