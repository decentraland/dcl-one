> Teleport the player, or walk them to a new position inside your scene

# Move the Player

To change the player's position in the scene, use the `movePlayerTo()` function. Use it to respawn players, set up checkpoints, or make a player walk to a spot as part of a cutscene.


> [!NOTE]
> **💡 Tip**: The easiest way to move the player is to use the [Scene Editor in Creator Hub](/docs/creator/scene-editor/get-started/about-editor/). Use the no-code **Move player** or the **Move player here** Actions, see [Make any item smart](/docs/creator/scene-editor/interactivity/make-any-item-smart/).


This function takes an object with the following properties:

* `newRelativePosition`: Where to position the player, expressed as a Vector3.
* `cameraTarget`: (optional) The direction to make the camera face, expressed as a Vector3 representing the coordinates of a point in space to look at. If no value is provided, the camera will maintain the same rotation as before moving.
* `avatarTarget`: (optional) The direction to make the avatar face, expressed as a Vector3 representing the coordinates of a point in space to look at. If no value is provided, the avatar will maintain the same rotation as before moving. If the player is in 1st person camera mode, the camera and avatar rotation are the same.
* `duration`: (optional) How long the transition should take, in seconds. If no value is provided, the transition will occur instantly. If a duration is provided, the avatar will walk or run to this new position.


> [!WARNING]
> **📔 Note**: During the transition, the avatar is not affected by colliders, so it can pass through objects.
> 



```ts
import { movePlayerTo } from '~system/RestrictedActions'

// create entity
const myEntity = engine.addEntity()
MeshRenderer.setBox(myEntity)
MeshCollider.setBox(myEntity)

Transform.create(myEntity, {
	position: { x: 4, y: 1, z: 4 },
})

// give entity behavior
pointerEventsSystem.onPointerDown(
	{
		entity: myEntity,
		opts: { button: InputAction.IA_POINTER, hoverText: 'Click' },
	},
	function () {
		// respawn player
		movePlayerTo({
			newRelativePosition: Vector3.create(1, 0, 1),
			cameraTarget: Vector3.create(8, 1, 8),
			avatarTarget: Vector3.create(8, 1, 8),
		})
	}
)
```

The player's movement occurs instantly, without any confirmation screens or camera transitions.


> [!WARNING]
> **📔 Note**: Players can only be moved if they already are standing inside the scene's bounds, and can only be moved to locations that are inside the limits of the scene's bounds. You can't use `movePlayerTo()` to transport a player to another scene. To move a player to another scene, see [Teleports](/docs/creator/sdk7/interactivity/external-links/#teleports).


The `movePlayerTo()` function is awaitable, so if the movement has a duration, you can use `await` to wait for the player to reach their destination.

```ts
import { movePlayerTo } from '~system/RestrictedActions'

pointerEventsSystem.onPointerDown(
	{
		entity: myEntity,
		opts: { button: InputAction.IA_POINTER, hoverText: 'Click' },
	},
	async function () {
		await movePlayerTo({
			newRelativePosition: Vector3.create(1, 0, 1),
			cameraTarget: Vector3.create(8, 1, 8),
			avatarTarget: Vector3.create(8, 1, 8),
			duration: 2,
		})
	}
)
```

If the player attempts to move during the duration of their transition, the transition will be interrupted and the `movePlayerTo` function will never resolve. You can prevent this behavior by disabling all inputs while the transition is in progress using the `InputModifier` component, see [Restrict locomotion](/docs/creator/sdk7/interactivity/avatars/locomotion/#restrict-locomotion).

```ts
import { movePlayerTo } from '~system/RestrictedActions'
import {InputModifier, engine} from '@dcl/sdk/ecs'

pointerEventsSystem.onPointerDown(
	{
		entity: myEntity,
		opts: { button: InputAction.IA_POINTER, hoverText: 'Click' },
	},
	async function () {

		// disable all inputs
		InputModifier.create(engine.PlayerEntity, {
			mode: InputModifier.Mode.Standard({
				disableAll: true,
			}),
		})

		// move player
		await movePlayerTo({
			newRelativePosition: Vector3.create(1, 0, 1),
			cameraTarget: Vector3.create(8, 1, 8),
			avatarTarget: Vector3.create(8, 1, 8),
			duration: 2,
		})

		// enable all inputs
		InputModifier.deleteFrom(engine.PlayerEntity)
	}
)
```


> [!WARNING]
> **📔 Note**: Additionally the result of the await can be stored to evaluate its `success` property to know if the movement was interrupted (e.g. by input movement) or completed successfully.



> [!NOTE]
> **💡 Tip**: To move a player to a different scene, or to different coordinates in the Genesis City map, see [Teleports](/docs/creator/sdk7/interactivity/external-links/#teleports).

