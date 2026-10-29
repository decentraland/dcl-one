> How to set the position, rotation and scale of an entity in a scene

# Entity Positioning

You can set the _position_, _rotation_ and _scale_ of any entity by using the `Transform` component. This can be used on any entity in the 3D space, affecting where the entitiy is rendered. This includes primitive shapes (cube, sphere, plane, etc), 3D text shapes, NFT shapes, and 3D models (`GltfContainer`).

## Use the Scene Editor in Creator Hub

When adding an item to your scene via the Scene Editor, it implicitly includes a **Transform** component. You then change the values in the entity's Transform component implicitly by changing the position, rotation or scale of an entity. You can also use the Scene Editor's UI to provide values numerically for more precision.

## Code essentials

![](/docs/creator/images/media/ecs-simple-components-new.webp)

```ts
// Create a new entity
const ball = engine.addEntity()

// Give this entity a shape, to make it visible
MeshRenderer.setSphere(ball)

// Give this entity a Transform component
Transform.create(ball, {
	position: Vector3.create(5, 1, 5),
	scale: Vector3.create(1, 1, 1),
	rotation: Quaternion.Identity(),
})
```

To move, rotate or resize an entity in your scene over a period of time, change the values on this component incrementally, frame by frame. See [Move entities](/docs/creator/sdk7/3d-essentials/move-entities/) for more details and best practices.


> [!WARNING]
> **📔 Note**: `Vector3` and `Quaternion` must be imported via
> 
> > `import { Vector3, Quaternion } from "@dcl/sdk/math"`
> 
> See [Imports](/docs/creator/sdk7/getting-started/coding-scenes/#imports) for how to handle these easily.


## Position

`position` is a _3D vector_, it sets the position of the entity's center on all three axes, _x_, _y_, and _z_. See [Geometry types](/docs/creator/sdk7/3d-essentials/special-types/) for more details.

```ts
// Create a new entity
const ball = engine.addEntity()

// Create transform with a predefined position
Transform.create(ball, {
	  position: Vector3.create(5, 1, 5)
})

// Fetch a mutable version of the transform
const mutableTransform = Transform.getMutable(ball)

// Set the position with an object
mutableTransform.position = { x: 5, y: 1, z: 5 }

// Set the position with an object (alternative syntax)
mutableTransform.position = Vector3.create(2, 1, 4)

// Set each axis individually
mutableTransform.position.x = 3
mutableTransform.position.y = 1
mutableTransform.position.z = 3
```

When setting a position, keep the following considerations in mind:

* The numbers in a position vector represent _meters_ (unless the entity is a child of a scaled entity).
* A scene that is made up of a single parcel measures 16m x 16m. The center of the scene (at ground level) is at `x:8, y:0, z:8`. If the scene is made up of multiple parcels, then the center will vary depending on their arrangement.
*   `x:0, y:0, z:0` refers to the _South-West_ corner of the scene's base parcel, at ground level.

    > Tip: When viewing a scene preview, a compass appears in the (0,0,0) point of the scene with labels for each axis as reference.

    > Note: You can change the base parcel of a scene by editing the `base` attribute of _scene.json_.
* To better orient yourself, use your _left_ hand:
  * your index finger (pointing forward) is the _z_ axis
  * your middle finger (pointing sideways) is the _x_ axis
  * your thumb (pointing up) is the _y_ axis.
* If an entity is a child of another, then `x:0, y:0, z:0` refers to the center of its parent entity, wherever it is in the scene.
*   Every entity in your scene must be positioned within the bounds of the parcels it occupies at all times. If an entity leaves these boundaries, it will raise an error.

    > Tip: When viewing a scene in preview mode, entities that are out of bounds are highlighted in _red_.
* Your scene is also limited in height. The maximum height is 330 meters for every scene, regardless of how many parcels it occupies. Entities placed above 200 meters may suffer multiplayer sync issues. See [scene limitations](/docs/creator/sdk7/optimizing/scene-limitations/) for more details.

## Rotation

`rotation` is stored as a [_quaternion_](https://en.wikipedia.org/wiki/Quaternion), a system of four numbers, _x_, _y_, _z_ and _w_. Each of these numbers goes from -1 to 1. See [Geometry types](/docs/creator/sdk7/3d-essentials/special-types/) for more details.

```ts
// Create a new entity
const cube = engine.addEntity()

// Create transform with a predefined rotation of 0
Transform.create(cube, {
	  rotation: Quaternion.Identity()
})

// Fetch a mutable version of the transform
const mutableTransform = Transform.getMutable(cube)

// Set the rotation with an object, from euler angles
mutableTransform.rotation = Quaternion.fromEulerDegrees(0, 90, 0)

// Set the rotation with an object
mutableTransform.rotation = { x: 0.1, y: 0.5, z: 0.5, w: 0 }

// Set each axis individually
mutableTransform.rotation.x = 0
mutableTransform.rotation.y = 1
mutableTransform.rotation.z = 0.3
mutableTransform.rotation.w = 0
```

You can also set the rotation field with [_Euler_ angles](https://en.wikipedia.org/wiki/Euler_angles), the more common _x_, _y_ and _z_ notation with numbers that go from 0 to 360 that most people are familiar with. To use Euler angles, use one of the following notations:

```ts
// Create transform with a predefined rotation in Euler angles
Transform.create(cube, {
	  rotation: Quaternion.fromEulerDegrees(0, 90, 0)
})

// Fetch a mutable version of the transform
const mutableTransform = Transform.getMutable(cube)

// Set the rotation with an object, from euler angles
mutableTransform.rotation = Quaternion.fromEulerDegrees(0, 90, 0)
```

When using a _3D vector_ to represent Euler angles, _x_, _y_ and _z_ represent the rotation in that axis, measured in degrees. A full turn requires 360 degrees.

When you retrieve the rotation of an entity, it returns a quaternion by default. To obtain the rotation expressed as in Euler angles, use `Quaternion.toEulerAngles()`:

```ts
// Fetch a read-only version of the transform
const transform = Transform.get(cube)

// Obtain the rotation expressed in euler angles
const eulerAngle = Quaternion.toEulerAngles(transform.rotation)
```

## Getting Global Position and Rotation of an Entity

The `getWorldPosition` and `getWorldRotation` functions return the global position and rotation of an entity. That means that it returns the percieved position or rotation that the player will see the item in, ignoring any parent hierarchies. 

* `getWorldPosition(engine, entity: Entity): Vector3Type`: This function returns the World position of an entity, having in consdieration all the positions of the parent entities, if the entity itself has any, returning `{x: 0, y: 0, z: 0}` if the entity has no Transform.

```ts
const worldPos = getWorldPosition(engine, childEntity)
console.log(`World position: ${worldPos.x}, ${worldPos.y}, ${worldPos.z}`)
```

* `getWorldRotation(engine, entity: Entity): QuaternionType`: This function returns the World rotation of an entity, having in consideration all the rotations of the parent entities, if the antity itself has any. It returns a `Quaternion` type, returning identity quaternion `{x: 0, y: 0, z: 0, w: 1}` if the entity has no Transform.

```ts
const worldRot = getWorldRotation(engine, childEntity)
console.log(`World rotation: ${worldRot.x}, ${worldRot.y}, ${worldRot.z}, ${worldRot.w}`)
```


> [!NOTE]
> **Note:** Global position and global rotation are relative to the coordinates inside the scene and not to Genesis City. 


## Face the player

Add a _Billboard_ component to an entity so that it always rotates to face the player.

Billboards were a common technique used in 3D games of the 90s, where most entities were 2D planes that always faced the player. The same idea can also be used to rotate a 3D model.

```ts
// Create a new entity
const cube = engine.addEntity()

// Give the entity a visible shape
MeshRenderer.setBox(cube)

// Create transform with a predefined position
Transform.create(cube, {
	  position: Vector3.create(5, 1, 5)
})

// Give the entity a Billboard component
Billboard.create(cube, {})
```

You can configure how the billboard behaves with the following parameters:

* `billboardMode`: Uses a value of the `BillboardMode` to set which rotation axes automatically rotate to face the player:
  * `BillboardMode.BM_ALL`: The entity rotates to face the player on all of its rotation axes. If the player is high above the entity, the entity will face up.
  * `BillboardMode.BM_NONE`: The entity won't rotate at all.
  * `BillboardMode.BM_X`: The entity only rotates on its _x_ rotation axis.
  * `BillboardMode.BM_Y`: The entity only rotates on its _y_ rotation axis. It only rotates left and right, not up and down. It stays perpendicular to the ground if the player is above or below the entity.
  * `BillboardMode.BM_Z`: The entity only rotates on its _z_ rotation axis.

* `targetEntity`: (optional) An entity that the billboard rotates to face, instead of the player's camera. If the referenced entity doesn't exist (for example, it hasn't been created yet or it was removed), the billboard stops rotating and keeps its last orientation until the entity exists again. Setting this to `engine.CameraEntity` is equivalent to leaving it unset.

```ts
// flat billboard
const perpendicularPlane = engine.addEntity()

Transform.create(perpendicularPlane, {
	position: Vector3.create(8, 1, 8),
})

MeshRenderer.setPlane(perpendicularPlane)

Billboard.create(perpendicularPlane, {
	billboardMode: BillboardMode.BM_Y,
})

// text label
const textLabel = engine.addEntity()

Transform.create(textLabel, {
	position: Vector3.create(6, 1, 6),
})

TextShape.create(textLabel, {
	text: 'This text is always readable',
})

Billboard.create(textLabel)

// sign that faces another entity instead of the player
const sphere = engine.addEntity()

Transform.create(sphere, {
	position: Vector3.create(4, 2, 4),
})

MeshRenderer.setSphere(sphere)

const sign = engine.addEntity()

Transform.create(sign, {
	position: Vector3.create(8, 1, 8),
})

TextShape.create(sign, {
	text: 'Watching the sphere',
})

Billboard.create(sign, {
	targetEntity: sphere,
})
```


> [!NOTE]
> **💡 Tip**: Billboards are very handy to add to _text_ entities, since it makes them always legible.


The `rotation` value of the entity's `Transform` component doesn't change as the billboard follows players around.

If an entity has both a `Billboard` component and `Transform` component with `rotation` values, players will see the entity rotating as a billboard. If the billboard doesn't affect all axis, the remaining axis will be rotated according to the `Transform` component.


> [!WARNING]
> **📔 Note**: If there are multiple players present at the same time, each will see the entities with billboard mode facing them. Billboard rotations are calculated locally for each player, and don't affect what others see. This doesn't apply to billboards with a `targetEntity`: since the target's position is part of the scene, all players see those billboards facing the same way.


## Face a set of coordinates

For entity A to look at entity B:

```
1) Subtract the position of entity A from entity B to get a vector that describes the distance between them.
2) Normalize that vector, so it has a length of 1, maintaining its direction.
3) Use `Quaternion.lookRotation` to get a Quaternion rotation that describes rotating in that direction.
4) Set that Quaternion as the rotation of entity A
```

```ts
export function turn(entity: Entity, target: Vector3.ReadonlyVector3) {
	const transform = Transform.getMutable(entity)
	const difference = Vector3.subtract(target, transform.position)
	const normalizedDifference = Vector3.normalize(difference)
	transform.rotation = Quaternion.lookRotation(normalizedDifference)
}
```

## Scale

`scale` is also a _3D vector_, stored as a `Vector3` object, including the scale factor on the _x_, _y_ and _z_ axis. The shape of the entity scaled accordingly, whether it's a primitive or a 3D model.

The default scale is 1, so assign a value larger to 1 to stretch an entity or smaller than 1 to shrink it.

```ts
// Create a new entity
const ball = engine.addEntity()

// Create transform with a predefined position
Transform.create(ball, {
	  scale: Vector3.create(5, 5, 5)
})

// Fetch a mutable version of the transform
const mutableTransform = Transform.getMutable(ball)

// Set the scale with a Vector3

mutableTransform.scale = Vector3.create(2, 2, 2)

// Set the position with an object
mutableTransform.scale = { x: 5, y: 1, z: 5 }

// Set each axis individually
mutableTransform.scale.x = 3
mutableTransform.scale.y = 3
mutableTransform.scale.z = 2
```

## Inherit transformations from parent

When an entity is nested inside another, the child entities inherit components from the parents. This means that if a parent entity is positioned, scaled or rotated, its children are also affected. The position, rotation and scale values of children entities don't override those of the parents, instead these are compounded.

You assign an entity to be parent of another by setting the `parent` field on the child entity's `Transform` component.

If a parent entity is scaled, all position values of its children are also scaled.

```ts
// Create entities
const parentEntity = engine.addEntity()
const childEntity = engine.addEntity()

// Create a transform for the parent
Transform.create(parentEntity, {
	position: Vector3.create(3, 1, 1),
	scale: Vector3.create(0.5, 0.5, 0.5),
})

// Create a transform for the child, and assign it as a child
Transform.create(childEntity, {
	position: Vector3.create(0, 1, 0),
	parent: parentEntity,
})
```

In this example, the child entity will be scaled down to 0.5, since its parent has that scale. The child entity's position will also be relative to its parent. We have to add the parent's position plus that of the child. In this case, since the parent is scaled to half its size, the transformation of the child is also scaled down proportionally. In absolute terms, the child is positioned at `{ x: 3, y: 1.5, z: 1 }`. If the parent had a `rotation`, this would also affect the child's final position, as it changes the axis in which the child is shifted.

If a child entity has no `position` on its Transform, the default is `0,0,0`, which will leave it positioned at the same position as its parent.

You can use an invisible entity with no shape component as a parent, to wrap a set of other entities. This entity won't be visible in the rendered scene, but can be used to group its children and apply a transform to all of them.


> [!WARNING]
> **📔 Note**: Avoid creating a loop in the hierarchy, where an entity ends up being its own ancestor. The SDK detects these loops and logs a warning naming the entities involved. The affected entities are not positioned correctly until you fix the parenting.


## Attach an entity to an avatar

There are three methods to attach an entity to the player:

* Make it a child of the to the **Avatar Entity**
* Make it a child of the to the **Camera Entity**
* Use the **AvatarAttach component**

The simplest way to attach an entity to the avatar is to set the parent as the [reserved entity](/docs/creator/sdk7/architecture/entities-components/#reserved-entities) `engine.PlayerEntity`. The entity will then move together with the player's position.

```ts
let childEntity = engine.addEntity()

MeshRenderer.setCylinder(childEntity)

Transform.create(childEntity, {
	scale: Vector3.create(0.2, 0.2, 0.2),
	position: Vector3.create(0, 0.4, 0),
	parent: engine.PlayerEntity,
})
```

You can also set an entity to the [reserved entity](/docs/creator/sdk7/architecture/entities-components/#reserved-entities) `engine.CameraEntity`. When using the camera entity in first person, the attached entity will follow the camera's movements. This is ideal to keep something always in view, for example for keeping the 3D model of the gun always in view, even when the camera points up.

```ts
let childEntity = engine.addEntity()

MeshRenderer.setCylinder(childEntity)

Transform.create(childEntity, {
	scale: Vector3.create(0.2, 0.2, 0.2),
	position: Vector3.create(0, 0.4, 0),
	parent: engine.CameraEntity,
})
```

To attach an object to one of the avatar´s bones, and have it move together with the avatar's animations, add an `AvatarAttach` component to the entity.

You can pick different anchor points on the avatar, most of these points are linked to the player's armature and follow the player's animations. For example, when using the right hand anchor point the attached entity will move when the avatar waves or swings their arms while running, just as if the player was holding the entity in their hand.

```ts
// Attach to main player, if avatarId is not set, engine.PlayerEntity is used by default
AvatarAttach.create(myEntity, {
	anchorPointId: AvatarAnchorPointType.AAPT_NAME_TAG,
})

// Attach to a player by ID
AvatarAttach.create(myEntity, {
	avatarId: '0xAAAAAAAAAAAAAAAAA',
	anchorPointId: AvatarAnchorPointType.AAPT_NAME_TAG,
})
```

When creating an `AvatarAttach` component, pass an object with the following data:

* `avatarId`: _Optional_ The ID of the player to attach to. This is the same as the player's Ethereum address, for those players connected with an Ethereum wallet. If not speccified, it attaches the entity to the local player's avatar.
* `anchorPointId`: What anchor point on the avatar skeleton to attach the entity, using a value from the enum `AvatarAnchorPointType`.


> [!WARNING]
> **📔 Note**: If you want all players in the scene to see an object attached to the same player, for example for all to see that Player A picked up an object and holds it on their left hand, then you must provide a value to `avatarId`. If not specified then all players will see the object attached to their own avatars.


The following example places an enitiy attached to a particular avatar, for all other players to see it attached to that same avatar.

```ts
import { getPlayer } from '@dcl/sdk/src/players'
import { AvatarAnchorPointType, AvatarAttach, engine, Entity } from '@dcl/sdk/ecs'
import { syncEntity } from '@dcl/sdk/network'

async function attachToPlayer(){

 let userData = await getPlayer()
 console.log(userData)

 if (!userData || !userData.wearables) return

  let entity = engine.addEntity()

  AvatarAttach.create(entity, {
    avatarId: userData.userId,
    anchorPointId: AvatarAnchorPointType.AAPT_RIGHT_HAND,
  })

  // Other components

  syncEntity(entity, [AvatarAttach.componentId])

}
```

The following anchor points are available on the `AvatarAnchorPointType` enum:

* `AAPT_RIGHT_HAND`: Fixed on the player's right hand
* `AAPT_LEFT_HAND`: Fixed on the player's left hand
* `AAPT_HEAD`: Fixed to center of the player's head.
* `AAPT_NECK`: Fixed to the player's base of the neck.
* `AAPT_SPINE`: Fixed to the top section of the backbone.
* `AAPT_SPINE1`: Fixed to the mid section of the backbone.
* `AAPT_SPINE2`: Fixed to the lower section of the backbone.
* `AAPT_HIP`: Fixed to the hip bone.
* `AAPT_LEFT_SHOULDER`: Fixed to the left shoulder.
* `AAPT_LEFT_ARM`: Fixed to the left first arm bone, at the height of the shoulder.
* `AAPT_LEFT_FOREARM`: Fixed to the left forearm bone.
* `AAPT_LEFT_HAND_INDEX`: Fixed to the tip of the left index finger.
* `AAPT_RIGHT_SHOULDER`: Fixed to the right shoulder.
* `AAPT_RIGHT_ARM`: Fixed to the right first arm bone, at the height of the shoulder.
* `AAPT_RIGHT_FOREARM`: Fixed to the right forearm bone.
* `AAPT_RIGHT_HAND_INDEX`: Fixed to the tip of the right index finger.
* `AAPT_LEFT_UP_LEG`: Fixed to the top leg bone on the left leg.
* `AAPT_LEFT_LEG`: Fixed to the bottom leg bone on the left leg.
* `AAPT_LEFT_FOOT`: Fixed to the ankle on the left leg.
* `AAPT_LEFT_TOE_BASE`: Fixed to the tip of the toe on the left leg.
* `AAPT_RIGHT_UP_LEG`: Fixed to the top leg bone on the right leg.
* `AAPT_RIGHT_LEG`: Fixed to the bottom leg bone on the right leg.
* `AAPT_RIGHT_FOOT`: Fixed to the ankle on the right leg.
* `AAPT_RIGHT_TOE_BASE`: Fixed to the tip of the toe on the right leg.
*   `.AAPT_NAME_TAG`: Floats right above the player's name tag, isn't affected by the player's animations.

    > Note: The name tag height is dynamically adjusted based on the height of the wearables a player has on. So a player wearing a tall hat will have their name tag a little bit higher than others.
*   `AAPT_POSITION` _DEPRECATED_: The player's overall position. This appears at a height of 0.8 above the player's feet.

    >


> [!WARNING]
> \> \*\*📔 Note\*\*: The \`AAPT\_POSITION\` is deprecated. To follow the player's overall position, it's best to make the entity a child of the to the Avatar Entity. See start of this section for an example. >



> [!NOTE]
> **💡 Tip**: To use these values, write `AvatarAnchorPointType.` and VS Code will display the full list of options on a dropdown.


![](/docs/creator/images/avatar-attach-points.webp)

Entity rendering is locally determined on each instance of the scene. Attaching an entity on one player doesn't make it visible to other players who are seeing that player. If an entity is attached to the default local player, each player will experience the entity as attached to their own avatar.


> [!WARNING]
> **📔 Note**: Entities attached to an avatar must stay within scene bounds to be rendered. If a player walks out of your scene, any attached entities stop being rendered until the player walks back in. Smart wearables don't have this limitation.


The `AvatarAttach` component overwrites the values in the `Transform` component. Any values you put in the `Transform` component are replaced by the relative position of the entity to that of the player's Transform, these values are updated frame by frame as the player moves and animates.

If you need to position an entity with an offset from the anchor point on the avatar, or a different rotation or scale, do this via a parent entity.

1. Create an invisible entity with just a `Transform` and an `AvatarAttach` component. Its `Transform` values will be overwritten as the player moves
2. Set the entity you want to attach as a child of this parent. Its `Transform` values can describe the offset from the anchor point.

```ts
// Create parent entity
const parentEntity = engine.addEntity()

// Attach parent entity to player
AvatarAttach.create(parentEntity, {
	anchorPointId: AvatarAnchorPointType.AAPT_NAME_TAG,
})

// Create child entity
let childEntity = engine.addEntity()

MeshRenderer.setCylinder(childEntity)

Transform.create(childEntity, {
	scale: Vector3.create(0.2, 0.2, 0.2),
	position: Vector3.create(0, 0.4, 0),
	parent: parentEntity,
})
```


> [!WARNING]
> **📔 Note**: If the attached entity has colliders, these colliders could block the player's movement or cause jittery effects. You may want to dissable the physics layer of the attached entity's colliders. See [Collision layers](/docs/creator/sdk7/3d-essentials/colliders/#collision-layers)



> [!WARNING]
> **📔 Note**: If you have a child entity and want to know the global Position and/or Rotation of it, you can use the `getWorldPosition` and `getWorldRotation` functions. You can check them in the [Getting Global Position and Rotation](#getting-global-position-and-rotation-of-an-entity) section.


### Attach to other players

You can use the `AvatarAttach` component to attach an entity to another player. To do this, you must know the player's id.

To attach an entity to the avatar of another player, you must provide the user's ID in the field `avatarId`. There are [various ways](/docs/creator/sdk7/interactivity/user-data/#get-player-data) to obtain this data.


> [!WARNING]
> **📔 Note**: For those players connected with an Ethereum wallet, their `userId` is the same as their Ethereum address.


Fetch the `userId` for all other nearby players via `getPlayer()`

```ts
executeTask(async () => {
	for (const [entity, data] of engine.getEntitiesWith(PlayerIdentityData)) {
		console.log('Player id: ', data.address)
	}
})
```

Using it together with `AvatarAttach`, you could use the following code to add a cube floating over the head of every other player in the scene:

```ts
executeTask(async () => {
        for (const [entity, data] of engine.getEntitiesWith(PlayerIdentityData)) {
            const myEntity = engine.addEntity()
            MeshRenderer.setBox(myEntity)
            AvatarAttach.create(myEntity, {
                anchorPointId: AvatarAnchorPointType.AAPT_LEFT_HAND,
                avatarId: data.address,
            })
        }
    })
```

See other ways to fetch other user's IDs in [Get Player Data](/docs/creator/sdk7/interactivity/user-data/#get-player-data).

## Scene boundaries

All entities in your scene must fit within the scene boundaries, as what's outside those boundaries is parcels of land that are owned by other players.

If any part of your models extend beyond these limits when running a preview, these parts that extend will be cut off and not rendered, both when running a preview and on the published scene.

The position of entities in your scene is constantly being checked as they move, if an entity leaves the scene and then returns it will be removed and then rendered normally again.

A grid on the scene's ground shows the limits of the scene, which by default rage from 0 to 16 on the _x_ and _z_ axis, and up to 20 on the _y_ axis. You're free to place entities underground, below 0 on the _y_ axis.


> [!NOTE]
> **💡 Tip**: If your scene needs more parcels, you can add them in the project's `scene.json` file. See [Scene metadata](/docs/creator/sdk7/projects/scene-metadata/) for instructions. Once added, you should see the grid extend to cover the additional parcels.

