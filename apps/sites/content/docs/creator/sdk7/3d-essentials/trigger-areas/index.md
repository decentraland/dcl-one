> Learn how to use trigger areas in your scene

# Trigger Areas

Trigger areas allow you to react to the event of a player entering or leaving an area, or of any other entity entering or leaving an area. This is a fundamental tool for creating interactive scenes. Use them for things like opening a door when the player approaches, or to score a point when a ball enters a goal.

## Using trigger areas

To use trigger areas you need to add a `TriggerArea` component to an entity, then use a `triggerAreaEventsSystem` to react to the events.

```ts
import { engine, Transform, TriggerArea, triggerAreaEventsSystem } from '@dcl/sdk/ecs'

// create entity
const triggerEntity = engine.addEntity()

// set Transform
Transform.create(triggerEntity, {
  position: Vector3.create(8, 0, 8)
  })

// Trigger area
TriggerArea.setBox(triggerEntity)

// Event when trigger area activated
triggerAreaEventsSystem.onTriggerEnter(triggerEntity, function(result) {
  if (result.trigger?.entity !== engine.PlayerEntity) return;
  console.log('Player entered trigger area!')
})
```

By default, the `TriggerArea` component reacts to the event of _any_ player walking into the area. The above code adds `if (result.trigger?.entity !== engine.PlayerEntity) return` to check that the entity that caused the event is the current player, and not someone else's avatar

## Trigger area shapes

Trigger areas can be either a box or a sphere.

```ts
import { engine, Transform, TriggerArea } from '@dcl/sdk/ecs'

// Box
TriggerArea.setBox(triggerEntity)

// Sphere
TriggerArea.setSphere(triggerEntity)
```


> [!NOTE]
> **💡 Tip**: The sphere is the easiest shape to calculate for the engine, as it's achieved by checking the distance from the center of the sphere. If in doubt, use a sphere.


To alter the size of the trigger area, you can use the `scale` property of the `Transform` component on the entity holding the `TriggerArea`.

```ts
import { engine, Transform, TriggerArea } from '@dcl/sdk/ecs'

const triggerEntity = engine.addEntity()

TriggerArea.setBox(triggerEntity)

Transform.create(triggerEntity, {
  scale: Vector3.create(4, 2, 4),
})
```

### Debugging

To debug your scene and see the area covered by the trigger area, you can add a `MeshRenderer` component to the entity with the trigger area, and set the shape to the one you want to debug. The dimensions of the default mesh will match the dimensions of the trigger area.

```ts
import { engine, Transform, TriggerArea } from '@dcl/sdk/ecs'

const triggerEntity = engine.addEntity()

TriggerArea.setBox(triggerEntity)

MeshRenderer.setBox(triggerEntity)

Transform.create(triggerEntity, {
  position: Vector3.create(8, 0, 8),
})
```

## Trigger area events

You can use the `triggerAreaEventsSystem` to react to the different events of a trigger area:

- `onTriggerEnter`: Triggered when an entity enters the trigger area.
- `onTriggerExit`: Triggered when an entity leaves the trigger area.
- `onTriggerStay`: Triggered while an entity is in the trigger area, every frame.

```ts
import { engine, Transform, TriggerArea, triggerAreaEventsSystem } from '@dcl/sdk/ecs'

const triggerEntity = engine.addEntity()

TriggerArea.setBox(triggerEntity)

// On enter
triggerAreaEventsSystem.onTriggerEnter(triggerEntity, function(result) {
  console.log('Player entered trigger area!')
})

// On exit
triggerAreaEventsSystem.onTriggerExit(triggerEntity, function(result) {
  console.log('Player exited trigger area!')
})

// On stay
triggerAreaEventsSystem.onTriggerStay(triggerEntity, function(result) {
  console.log('Player is in trigger area!')
})
```

## Trigger event responses

When a trigger area event is triggered, you can use the `result` parameter to get information about both the entity that was triggered and the entity that triggered the event.

The following properties are available in the `result` parameter:

- `triggeredEntity`: The ID of the entity that was triggered (this is the entity that owns the trigger area)
- `triggeredEntityPosition`: The position of the entity that was triggered
- `triggeredEntityRotation`: The rotation of the entity that was triggered
- `eventType`: The type of trigger event (ENTER, EXIT, STAY)
- `timestamp`: The timestamp of the trigger event
- `trigger`: An object with the following fields:
  - `entity`: The ID of the entity that triggered the trigger (the entity that entered the trigger area)
  - `layers`: The collision layers of the entity that triggered the trigger
  - `position`: The position of the entity that triggered the trigger
  - `rotation`: The rotation of the entity that triggered the trigger
  - `scale`: The scale of the entity that triggered the trigger


> [!WARNING]
> **Common confusion**: despite its name, `result.triggeredEntity` does **not** refer to the entity that triggered the event. These two fields are easily mixed up:
> 
> - `result.trigger?.entity`: the entity that walked into the trigger area. This is usually what you want to check, for example to identify which player or object entered.
> - `result.triggeredEntity`: the entity that owns the `TriggerArea` component (the trigger zone itself, not the entity that entered it).
> 
> Use `result.trigger?.entity` when you need to identify the entering entity.


```ts
import { engine, Transform, TriggerArea, triggerAreaEventsSystem } from '@dcl/sdk/ecs'

// Entity with the trigger area
const triggerEntity = engine.addEntity()

TriggerArea.setBox(triggerEntity)

Transform.create(triggerEntity, {
  position: Vector3.create(8, 0, 8),
})

// On enter
triggerAreaEventsSystem.onTriggerEnter(triggerEntity, function(result) {
  console.log('An entity entered trigger area!', result.triggeredEntity)
  console.log('Triggered entity position: ', result.triggeredEntityPosition)
  console.log('Triggered entity rotation: ', result.triggeredEntityRotation)
  console.log('Event type: ', result.eventType)
  console.log('Timestamp: ', result.timestamp)
  console.log('Trigger entity: ', result.trigger.entity)
  console.log('Trigger layers: ', result.trigger.layers)
  console.log('Trigger position: ', result.trigger.position)
  console.log('Trigger rotation: ', result.trigger.rotation)
  console.log('Trigger scale: ', result.trigger.scale)
})
```

## Detect players entering an area

The most common use of trigger areas is to react to players walking in or out. Two collision layers detect avatars, and which one to use depends on who the scene should react to:

- `ColliderLayer.CL_MAIN_PLAYER`: Detects **only the player using the local machine**. Use this when the response is meant for that player alone, for example to teleport them, play a sound for them, or track their progress in a quest. This is also the cheapest option in terms of performance.
- `ColliderLayer.CL_PLAYER`: Detects **all avatars**, both the local player and any other players being rendered in the scene. Use this when anyone walking in matters, for example a door that opens when anyone approaches, or counting how many people are in a room. This is the default layer if none is specified.

The following example reacts only to the local player:

```ts
import { engine, Transform, TriggerArea, ColliderLayer, triggerAreaEventsSystem } from '@dcl/sdk/ecs'

// create entity
const triggerEntity = engine.addEntity()

// set Transform
Transform.create(triggerEntity, {
  position: Vector3.create(8, 0, 8),
})

// Trigger area that only detects the local player
TriggerArea.setBox(triggerEntity, ColliderLayer.CL_MAIN_PLAYER)

// Event when trigger area activated
triggerAreaEventsSystem.onTriggerEnter(triggerEntity, function (result) {
  console.log('You entered the trigger area!')
})
```

When using `CL_PLAYER`, all avatars activate the trigger area. To tell whether the avatar that triggered it is the local player, compare the entity in the result against `engine.PlayerEntity`:

```ts
import { engine, Transform, TriggerArea, triggerAreaEventsSystem } from '@dcl/sdk/ecs'

// create entity
const triggerEntity = engine.addEntity()

// set Transform
Transform.create(triggerEntity, {
  position: Vector3.create(8, 0, 8),
})

// Trigger area, using the default CL_PLAYER layer
TriggerArea.setBox(triggerEntity)

// Event when trigger area activated
triggerAreaEventsSystem.onTriggerEnter(triggerEntity, function (result) {
  if (result.trigger?.entity === engine.PlayerEntity) {
    console.log('You entered the trigger area!')
  } else {
    console.log('Another player entered the trigger area!')
  }
})
```

In most cases, rather than trying to detect all players, the ideal approach is to detect only the current player (via `CL_MAIN_PLAYER`) and then sync the effects of this trigger between all players. For example, a sliding door opens only for your avatar, but then the open state of that door gets shared to all other players.


> [!NOTE]
> **💡 Tip**: Trigger areas whose mask only includes avatar layers (`CL_PLAYER`, `CL_MAIN_PLAYER`, or both) are optimized: the engine places them on an avatars-only physics layer, so overlaps with anything other than an avatar are discarded by the physics engine before they reach your scene's code. A trigger area with a mask exactly equal to `CL_MAIN_PLAYER` is optimized even further: it short-circuits any overlap with non-local-player colliders before reaching the handler, so detecting only the local player is essentially free.


## Trigger area layers

Use the optional second argument of the `TriggerArea` component to set the layers that will activate the trigger area. By default, the trigger area is activated only by the layer `ColliderLayer.CL_PLAYER`, see [Detect players entering an area](#detect-players-entering-an-area).

You can change the collision layer to detect any other entity by passing it as the second argument of the `TriggerArea` component.

```ts
import { engine, Transform, TriggerArea, MeshCollider, ColliderLayer, triggerAreaEventsSystem } from '@dcl/sdk/ecs'

// Trigger area
const triggerEntity = engine.addEntity()

TriggerArea.setBox(triggerEntity, ColliderLayer.CL_CUSTOM1)

Transform.create(triggerEntity, {
  position: Vector3.create(8, 0, 8),
})

// Entity that will activate the trigger area
const movingEntity = engine.addEntity()

Transform.create(movingEntity, {
  position: Vector3.create(8, 0, 8),
})

MeshCollider.setBox(movingEntity, ColliderLayer.CL_CUSTOM1)
```

Allowed values are the same as the ones for the `MeshCollider` component. See [Collision layers](/docs/creator/sdk7/3d-essentials/colliders/#Collision-layers) for more details.

- `ColliderLayer.CL_PLAYER`: any avatar (local + remote)
- `ColliderLayer.CL_MAIN_PLAYER`: only the local player
- `ColliderLayer.CL_PHYSICS`
- `ColliderLayer.CL_POINTER`
- `ColliderLayer.CL_CUSTOM1` through to `CL_CUSTOM8`
- `ColliderLayer.CL_NONE`


> [!NOTE]
> **💡 Tip**: The layers `CL_CUSTOM1` through to `CL_CUSTOM8` don't have any special behavior on their own, you can use them for whatever suits your scene best.


You can also set up a trigger area to detect multiple layers at once.

```ts
import { engine, Transform, TriggerArea, ColliderLayer, triggerAreaEventsSystem } from '@dcl/sdk/ecs'

const triggerEntity = engine.addEntity()

TriggerArea.setBox(triggerEntity, ColliderLayer.CL_CUSTOM1 | ColliderLayer.CL_CUSTOM2)

Transform.create(triggerEntity, {
  position: Vector3.create(8, 0, 8),
})
```

This will activate the trigger area when any entity with the layers `CL_CUSTOM1` or `CL_CUSTOM2` enters the trigger area.


> [!NOTE]
> **💡 Tip**: For a working example that exercises `TriggerArea` end-to-end, see the [`75,-9-trigger-areas`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/75,-9-trigger-areas) test scene, which covers the ENTER/EXIT wire contract and the SDK's per-tick `onTriggerStay` synthesis, rotated areas, moving triggerers on custom collider layers, and areas attached to falling entities. To see how the collider layers themselves behave, see [`5,5-collider-layers`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/5,5-collider-layers).

