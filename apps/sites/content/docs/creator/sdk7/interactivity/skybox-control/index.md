> Change the skybox time

# Skybox Control

You can change how a player sees the skybox whenever they are standing in your scene, this also affects the hue and direction of the global lighting.

The sky in Decentraland follows a default day/night cycle that takes 2 hours to complete, so there are 12 full cycles every real day. If the scene is not enforcing any fixed time of day, then players are also able to switch to a particular time of day by changing a slider in their UI.

Whenever players enter a scene with a different time of day, or the scene changes the time of day dynamically, the skybox transitions smoothly over a few seconds to this new value.

## Fixed time of day

You can set a fixed time of day for your scene. All players will see the scene with this time of day, and the skybox will not follow the day/night cycle.

In the Creator Hub, open the scene settings and click on the **Settings** tab to find the **Skybox** section. Uncheck the **Auto** option and set the time of day you want.

![](/docs/creator/images/fixed-time-of-day.webp)

You can also set the skybox time of day in your scene code. To do this, add the following section to your `scene.json` at root level:

```json
 "skyboxConfig": {
    "fixedTime": 36000
  }
```

The number refers to the number of seconds since the start of the day, ranging from 0 (that refers to _00:00_) to 86400 (that refers to _24:00_). Any number higher than 86400 is interpreted also as midnight.

Here are some more examples of valid values:

* 0 seconds => _00:00_
* 21600 seconds => _06:00_
* 43200 seconds => _12:00_
* 64800 seconds => _18:00_
* 86400 seconds => _24:00_

## Reading the time of day

You can read the time of day from your scene code using the `getWorldTime()` function.

```ts
import { getWorldTime } from '~system/Runtime'

executeTask(async () => {
  let time = await getWorldTime({})
  console.log(time.seconds)
})
```

The function returns a number between 0 and 86400, where 0 is midnight and 86400 is 24:00. This value is updated if the scene changes the time of day dynamically or if the player changes the time of day in the UI. Otherwise, it returns the value relative to the default day/night cycle.

## Changing the time of day dynamically

You can change the time of day dynamically using the `SkyboxTime` component. This component can only be added to the root entity of the scene `engine.RootEntity`.

```ts
import { SkyboxTime } from '@dcl/sdk/ecs'

function main() {
  SkyboxTime.create(engine.RootEntity, { fixedTime: 36000 })
}
```

The `fixed_time` property is a number between 0 and 86400, where 0 is midnight and 86400 is 24:00. Any number higher than 86400 is interpreted also as midnight.

Whenever this component is added, removed, or the `fixed_time` property is changed, the skybox time of day transitions smoothly over a few seconds to this new value. The same happens when the player steps out or into the scene. While the skybox time of day is fixed, the skybox will no longer follow progress in its day/night cycle and players can't change the time of day via the UI.

By default, the transition always happens in the forward direction, but you can change this by setting the `transitionMode` property to `TransitionMode.TM_FORWARD` or `TransitionMode.TM_BACKWARD`.

```ts
import { SkyboxTime, TransitionMode } from '@dcl/sdk/ecs'

function main() {
  SkyboxTime.create(engine.RootEntity, { fixedTime: 36000, transitionMode: TransitionMode.TM_BACKWARD })
}
```


> [!NOTE]
> **💡 Tip**: For working examples of skybox control, see the [`2,1-skybox-sdk-scene-a`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/2,1-skybox-sdk-scene-a) and [`3,1-skybox-sdk-scene-b`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/3,1-skybox-sdk-scene-b) test scenes, which drive `SkyboxTime` and `TransitionMode` live from a UI panel, and [`2,0-skybox-scene-json`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/2,0-skybox-scene-json), which reads the `fixedTime` set in `scene.json` back at runtime via `getSceneInformation()`.

