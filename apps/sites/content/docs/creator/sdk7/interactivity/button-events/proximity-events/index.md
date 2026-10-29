> Detect when a player enters or leaves the proximity of an entity.

# Proximity Events

Proximity interactions let entities react to button events when a player enters is near and roughly facing it, without requiring them to aim their cursor at the entity.

Traditional pointer events use ray-casting: the player must aim their cursor at an entity's collider before they can press a button to trigger the event. Proximity events instead check for entities that are inside a given area around the avatar, regardless of where the player's cursor is aiming at.

The interactive area considered for proximity interactions is a wide triangular slice of a sphere, projecting forward from the player's position. Unlike with pointer events, the direction that the avatar is facing is important, independently of the direction that the camera is facing.

To set more specific custom logic, you might want to deal with the raw data and use the [Advanced](/docs/creator/sdk7/interactivity/button-events/advanced-button-events/) approach.

For an entity to be interactive, it must have a [collider](/docs/creator/sdk7/3d-essentials/colliders/). See [obstacles](/docs/creator/sdk7/interactivity/button-events/click-events/#obstacles) for more details.


## Register proximity callbacks

The `pointerEventsSystem` includes helper functions for proximity events, following the same pattern as [pointer event callbacks](/docs/creator/sdk7/interactivity/button-events/register-callback/).

### Proximity button presses

Use `pointerEventsSystem.onProximityDown` to detect button presses while the player is within range. Unlike `onPointerDown`, this does not require the player to aim their cursor at the entity.

```ts
pointerEventsSystem.onProximityDown(
    {
        entity: myEntity,
        opts: {
            button: InputAction.IA_PRIMARY,
            hoverText: 'Press E',
            maxDistance: 5,
        },
    },
    function () {
        console.log('Player pressed button near entity')
    }
)
```

Use `pointerEventsSystem.onProximityUp` to detect when the player releases a button while in range.

```ts
pointerEventsSystem.onProximityUp(
    {
        entity: myEntity,
        opts: {
            button: InputAction.IA_PRIMARY,
            hoverText: 'Release E',
            maxDistance: 5,
        },
    },
    function () {
        console.log('Player released button near entity')
    }
)
```


> [!WARNING]
> **📔 Note**: Only one `onProximityDown` and one `onProximityUp` can be registered per entity. Once added, they keep listening until removed. Do not call these within a system loop, as that would keep rewriting the behavior.


## Proximity event types

Two new values have been added to the `PointerEventType` enum:

* `PET_PROXIMITY_ENTER`: Triggered when the player walks within the entity's proximity range.
* `PET_PROXIMITY_LEAVE`: Triggered when the player moves out of the entity's proximity range.

These can be used with a `PointerEvents` component and the `inputSystem` for the [system-based approach](/docs/creator/sdk7/interactivity/button-events/system-based-events/#proximity-events). That approach also supports proximity button presses using the `InteractionType.PROXIMITY` field.

## Priority

When multiple entities are within range and could respond to the same input, only one is activated, the closest one by default. Use the `priority` field to control which one takes precedence. Entities with a **higher number** as their priority value respond first.

If the player is both within the range of an entity with a proximity interaction and the player's cursor is pointing at an entity that has a pointer interaction, the entity with the pointer interaction is always given priority. Again, only one entity is activated.


```ts
pointerEventsSystem.onProximityDown(
    {
        entity: doorEntity,
        opts: {
            button: InputAction.IA_PRIMARY,
            hoverText: 'Open door',
            maxDistance: 5,
            priority: 2,
        },
    },
    function () {
        console.log('Door activated')
    }
)

pointerEventsSystem.onProximityDown(
    {
        entity: floorEntity,
        opts: {
            button: InputAction.IA_PRIMARY,
            hoverText: 'Step here',
            maxDistance: 5,
            priority: 1,
        },
    },
    function () {
        console.log('Floor activated')
    }
)
```

In the example above, if both entities are within range, the door entity responds because it has the higher priority.

The `priority` field can also be set in the properties of a `PointerEvents` component.

## Options

The proximity helper functions accept the same options as their pointer counterparts:

* `button`: Which button to listen for. See [Pointer buttons](/docs/creator/sdk7/interactivity/button-events/click-events/#pointer-buttons) for supported options.
* `maxDistance`: Maximum distance from the player's **avatar** to the entity, in meters. 10 by default.
* `hoverText`: Text to display in the UI when the player is near the entity.
* `showHighlight`: If true, shows an edge highlight on the entity when the player is in range. _true_ by default.
* `showFeedback`: If true, shows hover feedback, hovering around the center of the entity. _true_ by default.
* `priority`: Resolves conflicts when multiple entities are in proximity. Higher values take precedence. If multiple entities have the same priority value, the closest one is picked.

`maxPlayerDistance` is a deprecated alias of `maxDistance`. Both measure distance from the avatar, so use `maxDistance` in new scenes.

### What counts as being in range

Proximity events measure from the **center of the player's capsule**, roughly chest height, to the closest point on the entity's collider. That is about a meter above where a cursor click measures from, so the same spot can read as a slightly different distance on each path.

Distance alone is not enough. To fire a proximity event, the player must also:

* Be **facing** the entity, within a cone of roughly 120 degrees in front of them.
* Have a **clear line of sight** to it, with nothing in between.


> [!WARNING]
> **📔 Note**: Proximity events ignore `maxCameraDistance`. Where the camera sits makes no difference. Only the avatar's distance, facing, and line of sight are considered. See [Distance limits](/docs/creator/sdk7/interactivity/button-events/register-callback/#distance-limits) for how the camera limit works on cursor events.



## Remove callbacks

To remove a proximity callback, use the corresponding remove function:

```ts
pointerEventsSystem.removeOnProximityDown(myEntity)
pointerEventsSystem.removeOnProximityUp(myEntity)
pointerEventsSystem.removeOnProximityEnter(myEntity)
pointerEventsSystem.removeOnProximityLeave(myEntity)
```

Once removed, the entity is no longer reactive to that proximity event.


## Proximity enter and leave

Use `pointerEventsSystem.onProximityEnter` to run a callback when the player walks into the entity's proximity range, and `pointerEventsSystem.onProximityLeave` when they walk out. This can be used to display extra feedback hints to the player, for example a sound or an animation when an item can be interacted with.

```ts
pointerEventsSystem.onProximityEnter(
    {
        entity: myEntity,
        opts: {
            button: InputAction.IA_POINTER,
            hoverText: 'Nearby',
            maxDistance: 5,
        },
    },
    function () {
        console.log('Player entered proximity')
    }
)

pointerEventsSystem.onProximityLeave(
    {
        entity: myEntity,
        opts: {
            button: InputAction.IA_POINTER,
            hoverText: 'Nearby',
            maxDistance: 5,
        },
    },
    function () {
        console.log('Player left proximity')
    }
)
```


## Example: proximity door

The following example opens or closes a door when the player presses a button while standing nearby, without needing to aim at the door.

```ts
const doorPivot = engine.addEntity()
Transform.create(doorPivot, { position: Vector3.create(3, 0, 4) })

const door = engine.addEntity()
GltfContainer.create(door, { src: 'assets/door.glb' })
Transform.create(door, { position: Vector3.create(-1, 0, 0), parent: doorPivot })

let isDoorOpen = false
const closedRot = Quaternion.fromEulerDegrees(0, 0, 0)
const openRot = Quaternion.fromEulerDegrees(0, 90, 0)

pointerEventsSystem.onProximityDown(
    {
        entity: door,
        opts: {
            button: InputAction.IA_PRIMARY,
            hoverText: 'Open / Close',
            maxDistance: 5,
            priority: 1,
        },
    },
    function () {
        if (isDoorOpen) {
            Tween.setRotate(doorPivot, openRot, closedRot, 700)
            isDoorOpen = false
        } else {
            Tween.setRotate(doorPivot, closedRot, openRot, 700)
            isDoorOpen = true
        }
    }
)
```
