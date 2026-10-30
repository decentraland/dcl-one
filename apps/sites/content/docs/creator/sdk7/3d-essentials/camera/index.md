> Learn how to control the player's camera

# Camera

As a creator, you can have full control over the player's camera. By default, players are free to chose between a 1st or 3rd person camera mode while exploring your scene, but you can impose a different camera modality.

Virtual cameras can be static, they can rotate to always look at the player or some other entity, or they can be attached to the player or some other entity so that they're always accompanying.


> [!WARNING]
> **📔 Note**: To switch between the default 1st and 3rd person cameras, see [Camera modifier areas](#1st-and-3rd-person-camera-modes).


## 1st and 3rd person camera modes

Players are normally free to switch between first and third person camera by pressing V on the keyboard. Use a `CameraModeArea` to force the camera mode to either 1st or 3rd person for all players that stand within a specific area in your scene.

```ts
const entity = engine.addEntity()

CameraModeArea.create(entity, {
	area: Vector3.create(4, 3, 4),
	mode: CameraType.CT_FIRST_PERSON,
})
```

If a player's current camera mode doesn't match that of the `CameraModeArea`, they will transition to that camera mode. A toast appears onscreen to clarify that this change is due to the scene. While inside, players can't change their camera mode. When a player leaves the `CameraModeArea`, their camera mode is restored to what they had before entering.

Use `CameraModeArea` in regions where players would have a significantly better experience by using a specific camera mode. For example, first person is ideal if the player needs to click on small object, or third person may be useful for players to notice some entity that your scene has attached over their head. Don't assume players know how to switch camera modes, many first-time players might not know they have the option, or not remember the key to do it.


> [!WARNING]
> **📔 Note**: Camera modifier areas are affected by the _position_ and _rotation_ of the Transform component of their host entity, but they're not affected by the _scale_.



> [!WARNING]
> **📔 Note**: If you overlap multiple camera modifier areas, the last one to be instanced by your scene's code will take priority over the others.


When creating an `CameraModeArea` component, you must provide the following:

- `area`: Size of the modifier area
- `mode`: Which camera mode to force in this area, from the `CameraType` enum.

The supported camera modes are:

- `CameraType.CT_FIRST_PERSON`
- `CameraType.CT_THIRD_PERSON`

### Query the camera mode

You can query the camera mode of the player by using the `CameraMode` component on the `engine.CameraEntity`.

```ts
const cameraMode = CameraMode.get(engine.CameraEntity)
if (cameraMode.mode === CameraType.CT_FIRST_PERSON) {
	console.log('The player is using the 1st person camera')
} else {
	console.log('The player is using the 3rd person camera')
}
```

You can also subscribe to changes in the camera mode by using the `onChange` function on the `CameraMode` component.

```ts
CameraMode.onChange(engine.CameraEntity, (cameraMode) => {
	if (!cameraMode) return
	console.log('The player\'s camera mode changed to', cameraMode.mode)
})
```

## Using virtual cameras

To use a custom camera behavior in your scene, you need two things:

- Create a Virtual Camera: Create an entity in your scene and give it a `VirtualCamera`.
- Assign that virtual camera: Add a `MainCamera` component to the [reserved entity](/docs/creator/sdk7/architecture/entities-components/#reserved-entities) `engine.CameraEntity`, with a reference to the entity with the `VirtualCamera` component.

The camera will then be attached to the entity with the `VirtualCamera` component. If the entity moves or rotates, the camera moves with it.

```ts
function main() {
	const myCustomCamera = engine.addEntity()
	Transform.create(myCustomCamera, {
		position: Vector3.create(1, 2, 1),
	})
	VirtualCamera.create(myCustomCamera, {})

	const mainCamera = MainCamera.createOrReplace(engine.CameraEntity, {
		virtualCameraEntity: myCustomCamera,
	})
}
```

In this example, the camera will always be on a fixed position in the scene, as long as the player stays inside the scene bounds. As soon as the player steps outside the scene bounds, the default camera behavior will be restored.

### Field of view

You can override the Explorer's default field of view (FOV) for a virtual camera by setting the `fov` property, in degrees. The override applies only while this virtual camera is active. Omitting `fov` uses the Explorer's default (typically 60 degrees).

```ts
const cinematicCamera = engine.addEntity()
Transform.create(cinematicCamera, {
	position: Vector3.create(8, 3, 2),
})
VirtualCamera.create(cinematicCamera, {
	fov: 45,
})
```

A narrower FOV (lower value) zooms in, useful for cutscenes or aiming. A wider FOV (higher value) shows more of the scene, useful for panoramic views.

Your scene can include as many entities with a `VirtualCamera`component as you want, and dynamically switch between multiple virtual cameras as the player moves, or as they perform certain actions. Only one virtual camera is active at any given time, this is assigned by the `MainCamera` component on `engine.CameraEntity`.

To revert back to default camera behavior, set the value to `undefined` on `MainCamera.virtualCameraEntity`. The player is then free to switch between 1st and 3rd person cameras. If you want the player to only use one of these two modes, you can use a [Camera modifier areas](#1st-and-3rd-person-camera-modes) to force one of the two.


> [!WARNING]
> **📔 Note**: Camera modifier areas only have an effect on the player if no virtual cameras are active. If the scene is currently using a virtual camera and the player steps into a camera modifier area, nothing happens.
> 
> If a 3D model includes a `camera` node as part of its contents, this can't be used by the SDK. You must create all cameras as entities with the SDK.


```ts
function main() {
	// custom virtual camera
	const myCustomCamera = engine.addEntity()
	Transform.create(myCustomCamera, {
		position: Vector3.create(1, 2, 1),
	})
	VirtualCamera.create(myCustomCamera, {})

	const mainCamera = MainCamera.createOrReplace(engine.CameraEntity, {
		virtualCameraEntity: myCustomCamera,
	})

	// clickable cube
	const clickCube = engine.addEntity()
	Transform.create(clickCube, { position: Vector3.create(8, 0, 8) })
	MeshRenderer.setBox(clickCube)
	MeshCollider.setBox(clickCube)
	pointerEventsSystem.onPointerDown(
		{
			entity: clickCube,
			opts: { button: InputAction.IA_POINTER, hoverText: 'Reset camera' },
		},
		() => {
			// reset camera to default behavior
			const mainCamera = MainCamera.getMutable(engine.CameraEntity)
			mainCamera.virtualCameraEntity = undefined
		}
	)
}
```


> [!NOTE]
> **💡 Tip**: When the camera turns away from the avatar, it's often a good practice to also freeze the avatar's movements. That way the player doesn't move blindly into obstacles. See [Input Modifiers](/docs/creator/sdk7/interactivity/avatars/locomotion/#freeze-the-player)


To let the player steer a virtual camera with the mouse, read the `screenDelta` property of the `PrimaryPointerInfo` component to see how far the cursor moved on each frame, then apply that movement to the camera's rotation. This works even while the cursor is locked. See [Mouse Movement](/docs/creator/sdk7/interactivity/mouse-movement/) for a full mouselook example.

## Birds eye view

You can use a virtual camera to look at the scene from a top-down view, this can be a fun variation on the normal perspective of your avatar and enable different game mechanics.

You should avoid placing a camera looking in a perfect straight down direction. Instead, always tilt the camera slightly, even 1 degree is enough. This is because the player direction controls are based on the perspective of the camera, not on the avatar's orientation. If the camera is looking from a perfect birds-eye view, there is no clear definition of which way is which. An imperceptible tilt of 1 degree is enough to establish a forward direction.

```ts
function main() {
	// birds eye view camera
	const myCustomCamera = engine.addEntity()
	Transform.create(myCustomCamera, {
		position: Vector3.create(8, 5, 8),
		rotation: Quaternion.fromEulerDegrees(91, 0, 0),
		// Note that the rotation is 91º, not 90º
	})
	VirtualCamera.create(myCustomCamera, {})

	const mainCamera = MainCamera.createOrReplace(engine.CameraEntity, {
		virtualCameraEntity: myCustomCamera,
	})
}
```

## Field of View

You can set a custom field of view (FOV) on a virtual camera with the `fov` property. The value is in degrees. If omitted, it defaults to 60.

```ts
VirtualCamera.create(myCustomCamera, {
	fov: 90,
})
```

A wider FOV (higher value) shows more of the scene at once and creates a sense of speed, which can be useful for racing games. A narrower FOV (lower value) zooms in and is useful for aiming or cinematic shots.

## Camera Transitions

Whenever the scene switches between virtual cameras, or between the default camera behavior and virtual cameras, players see a transition. The position, rotation and any other parameters of the virtual camera change smoothly over a period time.

The transition settings on a virtual camera determine how you transition _into_ that camera, from any other camera in the scene, including the default. They don't affect how you transition _out_ of that camera.

```ts
VirtualCamera.create(myCustomCamera1, {
	defaultTransition: { transitionMode: VirtualCamera.Transition.Time(6) },
})
```


> [!NOTE]
> **💡 Tip**: To avoid having a transition, and switch instantly to a camera, set the transition time or speed to 0.


Depending on your use case, you may prefer to set the speed of the transition instead of the duration:

- **Fixed Time**: You set the duration of the transition, the camera will move as fast as it needs to complete the path in that period of time.
- **Fixed Speed**: You set how fast you want the virtual camera to move during the transition, the duration will depend on the distance. The value used for speed is interpreted as **meters per second**.

Below are examples for both these transition modes:

```ts
// fixed duration
VirtualCamera.create(myCustomCamera1, {
	defaultTransition: { transitionMode: VirtualCamera.Transition.Time(6) },
})

// fixed speed
VirtualCamera.create(myCustomCamera1, {
	defaultTransition: { transitionMode: VirtualCamera.Transition.Speed(3) },
})
```

Below is a full example with two virtual cameras, and transitions between them:

```ts
function main() {
	// custom virtual camera 1
	const myCustomCamera1 = engine.addEntity()
	Transform.create(myCustomCamera1, {
		position: Vector3.create(1, 2, 1),
	})
	VirtualCamera.create(myCustomCamera1, {
		defaultTransition: { transitionMode: VirtualCamera.Transition.Time(1) },
	})

	// custom virtual camera 2
	const myCustomCamera2 = engine.addEntity()
	Transform.create(myCustomCamera2, {
		position: Vector3.create(1, 2, 1),
	})
	VirtualCamera.create(myCustomCamera2, {
		defaultTransition: { transitionMode: VirtualCamera.Transition.Time(3) },
	})

	const mainCamera = MainCamera.createOrReplace(engine.CameraEntity, {
		virtualCameraEntity: myCustomCamera1,
	})

	// clickable cube
	const clickCube = engine.addEntity()
	Transform.create(clickCube, { position: Vector3.create(8, 0, 8) })
	MeshRenderer.setBox(clickCube)
	MeshCollider.setBox(clickCube)
	pointerEventsSystem.onPointerDown(
		{
			entity: clickCube,
			opts: { button: InputAction.IA_POINTER, hoverText: 'Reset camera' },
		},
		() => {
			// reset camera to default behavior
			const mainCamera = MainCamera.getMutable(engine.CameraEntity)
			mainCamera.virtualCameraEntity =
				mainCamera.virtualCameraEntity == myCustomCamera1
					? myCustomCamera2
					: myCustomCamera1
		}
	)
}
```

Transitions always move in a straight line, without considering any obstacles in the path. You could instead create a transition manually by using another virtual camera as an intermediary, that way you'd have full control over its movements. This intermediary virtual camera could perform a [Tween](/docs/creator/sdk7/3d-essentials/move-entities/#move-between-two-points) from the position of the first camera to the position of the second camera, or follow a more custom path that avoids obstacles or takes a cinematic detour.

## Camera following

You can configure a virtual camera so that it always faces the direction of the player, or some specific entity in the scene. The camera's position will remain static, but its rotation will change to always keep this entity centered.

This can be achieved with the `lookAtEntity` property in the `VirtualCamera` component. To follow the player, use the [reserved entity](/docs/creator/sdk7/architecture/entities-components/#reserved-entities) `engine.PlayerEntity`.

```ts
const myCustomCamera1 = engine.addEntity()
Transform.create(myCustomCamera1, {
	position: Vector3.create(1, 2, 1),
})
VirtualCamera.create(myCustomCamera1, {
	lookAtEntity: engine.PlayerEntity,
})
```

If an entity is being followed by the camera, this will only change the rotation, not the position of the camera.

As the camera rotates, the Transform of the entity with the `VirtualCamera` component does not change. However, you can read the camera's rotation from the Transform on `engine.CameraEntity`. The rotation and position of this entity will be absolute, it won't be conditioned by that of the entity with the `VirtualCamera` component. The rotation of this transform is affected as by the `lookAtEntity` behavior.


> [!WARNING]
> **📔 Note**: If you configure the virtual camera with a `lookAtEntity` that references the same entity that holds the virtual camera, or the `engine.CameraEntity` entity, the resulting behavior will be the same as not assigning any entity at all.


## Attach to the player

Another use of the virtual camera is to follow the player from a custom distance or angle, by attaching a virtual camera to the player entity. Note that the player can't change the camera's rotation freely, so in this case the camera's rotation will be fixed to that of the virtual camera. This could be useful for example for race games, where the player is expected to always look forward.

```ts
function main() {
	const myCustomCamera = engine.addEntity()
	Transform.create(myCustomCamera, {
		position: Vector3.create(0, 1, 5),
		parent: engine.PlayerEntity,
	})
	VirtualCamera.create(myCustomCamera, {
		defaultTransition: { transitionMode: VirtualCamera.Transition.Time(2) },
	})

	const mainCamera = MainCamera.createOrReplace(engine.CameraEntity, {
		virtualCameraEntity: myCustomCamera,
	})
}
```

## Cameras and colliders

When a player's camera moves in 3rd person mode, the camera might be blocked by colliders or not, depending on the collision layers assigned to the entities. Be mindful of this when designing your scene, you may want to prevent the camera from going through walls or other entities.

See [Colliders](/docs/creator/sdk7/3d-essentials/colliders/#cameras-and-colliders) for more details on how to configure colliders for your scene.

## Spectate mode (observer camera)

You can build a spectate mode that switches the player from normal avatar movement to a free-roaming or player-following camera. This is useful for observer roles in competitive games, director cameras for live events, or replay systems.

The pattern combines several SDK features:

| Feature                | SDK API                                   | Purpose                           |
| ---------------------- | ----------------------------------------- | --------------------------------- |
| Custom camera view     | `VirtualCamera` + `MainCamera`            | Replaces the player's camera      |
| Freeze avatar movement | `InputModifier` (`disableAll: true`)      | Frees WASD to drive the camera    |
| Track players in scene | `onEnterScene` / `onLeaveScene`           | Builds a roster of follow targets |
| Camera controls        | `inputSystem.isPressed(InputAction.IA_*)` | WASD for pitch/yaw, E/F for zoom  |
| Mouse-look             | `PrimaryPointerInfo.screenDelta`          | Rotate the camera with the mouse  |

### Camera rig architecture

Use a **two-entity rig** so yaw and pitch stay independent:

```
rigRoot (entity)           -- world position + yaw rotation
└── rigCamera (child)      -- pitch rotation + orbit offset
    └── VirtualCamera
```

`rigRoot` holds the yaw (left/right turn) and is lerped toward the follow target or a free-cam pivot. `rigCamera` handles pitch (up/down tilt) and the orbit distance from the root. Splitting yaw and pitch across two Transforms keeps the euler math straightforward.

### Enable and disable

```ts
// Activate spectate mode
const rigRoot = engine.addEntity()
Transform.create(rigRoot, {
  position: Vector3.create(8, 8, 8),
  rotation: Quaternion.fromEulerDegrees(0, 0, 0),
})

const rigCamera = engine.addEntity()
Transform.create(rigCamera, { parent: rigRoot })
VirtualCamera.create(rigCamera, {})

MainCamera.createOrReplace(engine.CameraEntity, { virtualCameraEntity: rigCamera })
InputModifier.createOrReplace(engine.PlayerEntity, {
  mode: InputModifier.Mode.Standard({ disableAll: true }),
})
```

```ts
// Deactivate spectate mode
// IMPORTANT: clear MainCamera BEFORE removing the VirtualCamera entity.
// If you remove the entity first, the engine keeps binding to a dead entity
// and the view falls to the player's feet.
const mainCamera = MainCamera.getMutableOrNull(engine.CameraEntity)
if (mainCamera) mainCamera.virtualCameraEntity = undefined

engine.removeEntity(rigCamera)
engine.removeEntity(rigRoot)

InputModifier.createOrReplace(engine.PlayerEntity, {
  mode: InputModifier.Mode.Standard({ disableAll: false }),
})
```


> [!CAUTION]
> **Warning:** Always clear `MainCamera.virtualCameraEntity` before removing the camera entity. Removing the entity first leaves the engine pointing at a dead reference, causing the view to break.


### Camera bounds

The engine disables `VirtualCamera` entities that move outside your scene's parcel bounds. If the camera leaves the scene footprint, it stops working silently. Clamp the camera position to stay inside the scene's axis-aligned bounding box (AABB) every frame, with a small margin.

```ts
// Example bounds for a 1x1 parcel scene (16m x 16m)
const BOUNDS_MIN = Vector3.create(0, 0, 0)
const BOUNDS_MAX = Vector3.create(16, 20, 16)
const BOUNDS_MARGIN = 0.5
```

For larger scenes, set the bounds to match your `scene.json` parcels. The scene height limit is 330 metres regardless of parcel count. A 4x4 parcel scene could use `Vector3.create(64, 330, 64)`.

### Following players

Use `onEnterScene` and `onLeaveScene` to build a live roster of players in your scene. Players can then cycle through follow targets with keys (for example, `IA_ACTION_3` and `IA_ACTION_4`). When following a player, the rig root lerps toward the followed player's Transform position, and the child camera orbits at a configurable distance.

```ts
import { onEnterScene, onLeaveScene } from '@dcl/sdk/src/players'

const playerEntities = new Map<string, Entity>()
let playerIds: string[] = []

onEnterScene((player) => {
  if (!player) return
  playerIds.push(player.userId)
  playerEntities.set(player.userId, player.entity)
})

onLeaveScene((userId) => {
  if (!userId) return
  playerIds = playerIds.filter((id) => id !== userId)
  playerEntities.delete(userId)
})
```

To follow a player each frame, lerp the rig root toward their position:

```ts
const followEntity = playerEntities.get(followTargetId)
if (followEntity) {
  const targetPos = Transform.get(followEntity).position
  const rootTransform = Transform.getMutable(rigRoot)
  rootTransform.position = Vector3.lerp(
    rootTransform.position,
    Vector3.add(targetPos, Vector3.create(0, 1, 0)),
    0.1
  )
}
```

See [Player enters or leaves scene](/docs/creator/sdk7/interactivity/event-listeners/#player-enters-or-leaves-scene) for more on `onEnterScene` and `onLeaveScene`.

### Mouse-look while spectating

Read `PrimaryPointerInfo.screenDelta` each frame to rotate the camera with the mouse. This works while the pointer is locked. See [Mouse Movement](/docs/creator/sdk7/interactivity/mouse-movement/) for a full mouselook example.

```ts
const MOUSE_SENSITIVITY = 0.15 // degrees per pixel
let yaw = 0
let pitch = 45

function spectateMouseLook() {
  const isLocked = PointerLock.getOrNull(engine.CameraEntity)?.isPointerLocked ?? false
  if (!isLocked) return

  const delta = PrimaryPointerInfo.getOrNull(engine.RootEntity)?.screenDelta
  if (!delta) return

  yaw = (yaw + delta.x * MOUSE_SENSITIVITY) % 360
  // Subtract delta.y so mouse-up tilts the camera up; clamp to prevent flip
  pitch = Math.max(-25, Math.min(80, pitch - delta.y * MOUSE_SENSITIVITY))
}
```


> [!WARNING]
> **Note:** `screenDelta` is desktop-only. On mobile it always reports `0`. If your scene targets mobile, design a touch-based fallback.


### Full reference implementation

The [`33,20-spectate-mode`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/33,20-spectate-mode) test scene contains a self-contained spectate module in `src/spectate.ts`. It supports free-cam and follow-cam modes, WASD + mouse-look controls, player cycling, orbit-distance zoom, bounds clamping, and an on-screen HUD showing controls and the current follow target.

To use it in your own project:

1. Copy `src/spectate.ts` into your scene.
2. Update `PIVOT`, `BOUNDS_MIN`, and `BOUNDS_MAX` at the top of the file to match your scene's parcels. This is the most common integration mistake.
3. Wire `toggleSpectate()` to any trigger: a clickable entity, a UI button, or a key press.


> [!NOTE]
> **💡 Tip**: For working examples of camera control, see the [`2,22-virtual-cameras`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/2,22-virtual-cameras) test scene, which cycles several `VirtualCamera` entities through `MainCamera` including `lookAtEntity` aimed at the player and a tweened camera; [`32,20-virtual-camera-mouse-look`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/32,20-virtual-camera-mouse-look), which drives a mouselook camera while the pointer is locked; [`33,20-spectate-mode`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/33,20-spectate-mode), which implements a full spectate/observer camera with follow-cam and free-cam modes; and [`9,99-modifier-areas`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/9,99-modifier-areas), which forces a camera mode inside a volume with `CameraModeArea`.

