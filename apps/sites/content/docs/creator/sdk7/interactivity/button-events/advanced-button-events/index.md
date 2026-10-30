> Learn how to handle user clicks in your scene.

# Advanced Button Events

If you need the interaction in your scene to follow custom logic that is not compatible with the [Register callback](/docs/creator/sdk7/interactivity/button-events/register-callback/) or the [System based](/docs/creator/sdk7/interactivity/button-events/system-based-events/) approaches, you can deal with the raw input event data directly. This approach is the hardest, but the most flexible.

## The PointerEventsResult component

When a pointer event is detected by the engine, the entity that was clicked is assigned a `PointerEventsResult` component. This component contains all the raw data about the hit event, and stores historic data about previous events.

The helpers in `inputSystem`, like `inputSystem.isTriggered` or `inputSystem.getInputCommand` are good for most simple scenarios, but if you need to get more details about the hit event, check the raw data in the `PointerEventsResult`.

The `PointerEventsResult` component holds a list of pointer events, containing one object for each event. It stores a list of up to 100 events, newer events are stored at the end of the list. Once the list reaches a length of 100, it starts discarding old events for each new one that comes in.

Each event in the list has the following data:

* `analog`: Optional number, only present on events from an analog input (like a joystick), storing the input's analog value.
* `button`: Which button id was pressed. The number corresponds to the `InputAction` enum, that lists all of the available buttons.
* `state`: Type of pointer event, from the enum `PointerEventType`. _0_ refers to `PointerEventType.PET_UP`, _1_ to `PointerEventType.PET_DOWN`, _2_ to `PointerEventType.PET_HOVER_ENTER`, _3_ to `PointerEventType.PET_HOVER_LEAVE`, _4_ to `PointerEventType.PET_PROXIMITY_ENTER`, _5_ to `PointerEventType.PET_PROXIMITY_LEAVE`
*   `timestamp`: A [lamport timestamp](https://en.wikipedia.org/wiki/Lamport_timestamp) to identify each button event.

    > Note: This timestamp is not numbered based on the current time. Think of it as a counter that starts at 0 and is incremented by 1 for each event.
* `hit`: An object that contains the following data about the hit event:
  * `entityId`: Id number of the entity that was hit by the ray.
  * `meshName`: _String_ with the internal name of the specific mesh in the 3D model that was hit. This is useful when a 3D model is composed of multiple meshes.
  * `globalOrigin`: _Vector3_ for the position where the ray originates (relative to the scene)
  * `direction`: _Vector3_ with the direction vector of the ray, in global coordinates
  * `position`: _Vector3_ for the position where the ray intersected with the hit entity (relative to the scene)
  * `length`: Length of the ray from its origin to the position where the hit against the entity occurred.
  * `normalHit`: _Vector3_ with a normalized direction vector, describing the angle of the normal of the hit in world space.

```ts
import { engine, PointerEventsResult } from '@dcl/sdk/ecs'

function PointerReadingSystem() {
  for (const [entity] of engine.getEntitiesWith(PointerEventsResult)) {
    const events = PointerEventsResult.get(entity)
    for (const event of events) {
      console.log('POINTER EVENT DATA:', event)
    }
  }
}

engine.addSystem(PointerReadingSystem)
```

## Track player movements

In real-time multiplayer games where the timing of player movements is critical, you may want to keep track of each player's position using a 3rd party server as the source of truth. You can improve response time by listening to the button in advance and predict their effects in your server before the avatar has shifted position.

This approach helps compensate for network delays, but is sure to result in discrepancies, so you should also regularly poll the player's current position to make corrections. Balancing these predictions and corrections may require plenty of fine-tuning.
