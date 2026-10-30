> Apply forces and impulses to the player's avatar

# Player Physics

You can apply physical forces to the player's avatar from your scene's code. This allows you to create gameplay mechanics like launch pads, wind zones, knockback effects, explosions, and more.

There are two kinds of force you can apply:

* **Impulse**: A one-shot instantaneous push. Use this for sudden discrete effects like launching the player into the air or knocking them back.
* **Continuous force**: A sustained push applied every tick for as long as it's active. Use this for ongoing effects like wind zones, water currents, or gravity fields.

Both are applied through the `Physics` helper, imported from `@dcl/sdk/ecs`.


> [!WARNING]
> **📔 Note**: These forces only affect the local player's avatar. Other players see the changes on other player's positions, but the forces themselves aren't synced to other players in multiplayer. Each player's physics run locally on their own instance.


## Apply an impulse

Use `Physics.applyImpulseToPlayer()` to give the player a one-shot push in a given direction.

* `vector`: Direction and strength combined — the length of the vector encodes the impulse magnitude.

```ts
import { Physics } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

// Launch the player straight up
Physics.applyImpulseToPlayer(Vector3.create(0, 50, 0))
```

You can also pass a direction and a magnitude separately. In this case, the direction vector is normalized automatically:

* `direction`: Direction to push — normalized automatically before scaling.
* `magnitude`: Impulse strength.

```ts
import { Physics } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

// Launch the player upward with a magnitude of 50
Physics.applyImpulseToPlayer(Vector3.create(0, 1, 0), 50)
```

If you call `applyImpulseToPlayer()` multiple times within the same frame, the impulses are accumulated — they are added together and applied as a single combined impulse.

### Trigger an impulse on player entry

A common pattern is to trigger an impulse when the player steps into an area. Use a trigger area to detect when the player enters:

```ts
import { engine, Physics, TriggerArea, triggerAreaEventsSystem, ColliderLayer, MeshRenderer, Transform } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

const launchPad = engine.addEntity()
Transform.create(launchPad, { position: Vector3.create(8, 0, 8) })
MeshRenderer.setBox(launchPad)
TriggerArea.setBox(launchPad, ColliderLayer.CL_PLAYER)

triggerAreaEventsSystem.onTriggerEnter(launchPad, (result) => {
	if (result.trigger?.entity !== engine.PlayerEntity) return
	Physics.applyImpulseToPlayer(Vector3.create(0, 50, 0))
})
```

## Apply a knockback impulse

Use `Physics.applyKnockbackToPlayer()` to push the player away from a point with a single impulse. This is ideal for explosions, impact effects, and other one-shot directional blasts. The direction is computed automatically from the source point to the player's current position.

```ts
import { Physics } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

const explosionPosition = Vector3.create(8, 1, 8)

// Knock the player away from the explosion center
Physics.applyKnockbackToPlayer(explosionPosition, 40)
```

You can limit the area of effect with a `radius`, and control how magnitude decreases with distance using the `KnockbackFalloff` option:

* `fromPosition`: World-space origin of the knockback (explosion center, enemy position, etc.).
* `magnitude`: Base impulse strength.
* `radius`: Max distance of effect (default: `Infinity`).
* `falloff`: How magnitude decreases with distance (default: `CONSTANT`).

```ts
import { Physics, KnockbackFalloff } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

Physics.applyKnockbackToPlayer(
	Vector3.create(8, 1, 8),
	40,                          // magnitude
	10,                          // radius: no effect beyond 10 meters
	KnockbackFalloff.LINEAR      // magnitude fades to 0 at the radius edge
)
```

The `KnockbackFalloff` enum controls how the force decreases with distance:

* `KnockbackFalloff.CONSTANT` — same magnitude at any distance within the radius (default)
* `KnockbackFalloff.LINEAR` — smooth linear decrease to 0 at the radius edge
* `KnockbackFalloff.INVERSE_SQUARE` — sharp physically-realistic drop-off

If the player is exactly at the source position, the player is pushed straight up. A negative magnitude pulls the player toward the point instead of pushing them away.

The same `KnockbackFalloff` values are available for `applyRepulsionForceToPlayer()`.

## Apply a continuous force

Use `Physics.applyForceToPlayer()` to apply a sustained force to the player. Unlike an impulse, this force is applied every tick as long as it remains active.

* `source`: An entity that identifies this force source — use it to update or remove the force later.
* `vector`: Direction and strength combined — the length of the vector encodes the force magnitude.

The source entity's position is not relevant — it's only used as an identifier.

```ts
import { engine, Physics } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

const windZoneEntity = engine.addEntity()

// Push the player to the right continuously
Physics.applyForceToPlayer(windZoneEntity, Vector3.create(10, 0, 0))
```

The force vector is always in **world space**. If you need a direction relative to a rotated entity, use `Transform.localToWorldDirection()` to convert it first — see [Convert a local direction to world space](#convert-a-local-direction-to-world-space).

As with `applyImpulseToPlayer()`, you can also pass a `direction` and `magnitude` separately — see [Apply an impulse](#apply-an-impulse):

```ts
Physics.applyForceToPlayer(windZoneEntity, Vector3.create(0, 1, 0), 50)
```

If you call `applyForceToPlayer()` again with the same source entity, it replaces the previous force from that source. If multiple force sources are accumulated their vectors are summed each tick.

### Remove a continuous force

To stop applying a force, call `Physics.removeForceFromPlayer()` with a reference to the source entity that was used to create the force. If the entity isn't currently applying a force, this call is safely ignored.

* `source`: The entity used when the force was applied.

```ts
Physics.removeForceFromPlayer(windZoneEntity)
```


### Apply a force for a limited duration

Use `Physics.applyForceToPlayerForDuration()` to apply a force for a specific amount of time. The duration is in seconds. The force is automatically removed when the time elapses. Calling this again with the same source entity resets the timer.

* `source`: An entity that identifies this force source.
* `duration`: How long the force lasts, in seconds.
* `vector`: Direction and strength combined — the length of the vector encodes the force magnitude.

```ts
import { engine, Physics } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

const gustEntity = engine.addEntity()

// Apply a strong upward force for 1.5 seconds
Physics.applyForceToPlayerForDuration(gustEntity, 1.5, Vector3.create(0, 50, 0))
```

The `direction` + `magnitude` overload is also available — see [Apply an impulse](#apply-an-impulse):

```ts
Physics.applyForceToPlayerForDuration(gustEntity, 1.5, Vector3.create(0, 1, 0), 50)
```

### Wind zone example

This example creates a wind tunnel that pushes the player while they're inside it, and stops when they leave:

```ts
import { engine, Physics, TriggerArea, triggerAreaEventsSystem, ColliderLayer, MeshRenderer, MeshCollider, Transform } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

const windTunnel = engine.addEntity()
Transform.create(windTunnel, {
	position: Vector3.create(8, 1, 8),
	scale: Vector3.create(4, 3, 4),
})
MeshRenderer.setBox(windTunnel)
TriggerArea.setBox(windTunnel, ColliderLayer.CL_PLAYER)

triggerAreaEventsSystem.onTriggerEnter(windTunnel, (result) => {
	if (result.trigger?.entity !== engine.PlayerEntity) return
	Physics.applyForceToPlayer(windTunnel, Vector3.create(15, 0, 0))  // push sideways along X axis
})

triggerAreaEventsSystem.onTriggerExit(windTunnel, (result) => {
	if (result.trigger?.entity !== engine.PlayerEntity) return
	Physics.removeForceFromPlayer(windTunnel)
})
```

## Apply a repulsion force

Use `Physics.applyRepulsionForceToPlayer()` to continuously push the player away from a fixed point in space. This is suited for effects like a magnetic repulsion field, a force barrier, or a hovering object that keeps players at a distance. Unlike `applyKnockbackToPlayer()`, this force is recalculated every tick as the player moves. The magnitude weakens with distance based on the radius and falloff — see [Apply a knockback impulse](#apply-a-knockback-impulse) for the `KnockbackFalloff` options. A negative magnitude reverses the effect, continuously pulling the player toward the point like a vortex or gravity well.


> [!WARNING]
> **📔 Note**: The repulsion origin is the `fromPosition` vector you pass — **not** the position of the `source` entity. The `source` entity is only used as an identifier so you can update or remove the force later. Its position, rotation, and scale are completely ignored.


* `source`: An entity used as an identifier for this force — its position is not used.
* `fromPosition`: The world-space point the player is pushed away from.
* `magnitude`: Base force strength. Negative values attract instead of repel.
* `radius`: Max distance of effect (default: `Infinity`).
* `falloff`: How magnitude decreases with distance (default: `CONSTANT`).

```ts
import { engine, Physics, Transform, timers } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

const repulsionSource = engine.addEntity()
Transform.create(repulsionSource, { position: Vector3.create(8, 1, 8) })

// Pass the entity's position explicitly as the repulsion origin
Physics.applyRepulsionForceToPlayer(
	repulsionSource,
	Transform.get(repulsionSource).position,  // repulsion origin — passed explicitly, not read automatically
	50,                                         // magnitude
	10,                                         // radius of effect in meters
)

// Remove force after half a second
timers.setTimeout(() => {
    Physics.removeForceFromPlayer(repulsionSource)
}, 500)
```


## Forces while gliding

While the player is gliding, continuous forces behave differently:

* Continuous forces — from `applyForceToPlayer()`, `applyForceToPlayerForDuration()`, or `applyRepulsionForceToPlayer()` — are **1.5 times stronger**, since the open glider catches the airflow. Wind zones and currents feel more responsive to a gliding player.
* The upward component of a continuous force can **lift** a gliding player. The glider's falling speed limit only caps how fast the player descends, it doesn't cancel upward motion, so an angled or vertical wind current pushes the player along the full direction of the force.

One-shot impulses — from `applyImpulseToPlayer()` or `applyKnockbackToPlayer()` — are not affected by gliding. They behave the same whether the glider is open or closed.

You can combine these behaviors to build mechanics like thermal updrafts that carry gliding players upward, or wind corridors that are easier to traverse with the glider open. To adjust the glider's falling speed or forward speed, use the `AvatarLocomotionSettings` component, see [Locomotion Settings](/docs/creator/sdk7/interactivity/avatars/locomotion/#locomotion-settings).

## Convert a local direction to world space

Use `Transform.localToWorldDirection()` to transform a direction vector from an entity's local coordinate space to world space, accounting for the full parent hierarchy. This is useful when applying forces relative to a rotated entity — for example, pushing the player away from a specific face of a rotating obstacle.

* `entity`: The source entity whose local space defines the direction.
* `localDirection`: Direction vector in the entity's local coordinates.

Returns the direction vector in world coordinates.

This applies **rotation only** — it does not account for translation or scale — making it directly suitable for direction vectors passed to force and impulse functions.

```ts
import { Physics, Transform } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

// Push the player in the direction the entity's local +Z axis points in world space
const worldDir = Transform.localToWorldDirection(myEntity, Vector3.create(0, 0, 1))
Physics.applyImpulseToPlayer(worldDir, 20)
```

<!--
## Local vs world space

By default, forces and impulses are applied in **world space** — the x, y, and z axes are fixed relative to the scene. You can instead apply them in **local space**, which is relative to the player's current facing direction. This is useful for mechanics like a "forward boost" that pushes the player in the direction they're looking.

To specify the space, pass a `PhysicsForceSpace` value as the last argument:

```ts
import { Physics, PhysicsForceSpace } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

// Push the player forward relative to where they're facing
Physics.applyImpulseToPlayer(
	Vector3.create(0, 5, 15),
	PhysicsForceSpace.PFS_LOCAL
)
```

```ts
import { Physics, PhysicsForceSpace } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

const boostEntity = engine.addEntity()

// Continuously push the player forward relative to their facing direction
Physics.applyForceToPlayer(
	boostEntity,
	Vector3.create(0, 0, 10),
	PhysicsForceSpace.PFS_LOCAL
)
```

The `PhysicsForceSpace` enum has two values:

* `PhysicsForceSpace.PFS_WORLD`: Default. Force is applied in world-space coordinates.
* `PhysicsForceSpace.PFS_LOCAL`: Force is rotated to match the player's current rotation before being applied.
 -->
