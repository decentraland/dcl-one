> Change the skybox time, replace the sky with your own texture, or recolor the sky, clouds and stars and control the sun and fog

# Skybox Control

You can change how a player sees the skybox whenever they are standing in your scene, this also affects the hue and direction of the global lighting.

The sky in Decentraland follows a default day/night cycle that takes 2 hours to complete, so there are 12 full cycles every real day. If the scene is not enforcing any fixed time of day, then players are also able to switch to a particular time of day by changing a slider in their UI.

Whenever players enter a scene with a different time of day, or the scene changes the time of day dynamically, the skybox transitions smoothly over a few seconds to this new value.

## Fixed time of day

You can set a fixed time of day for your scene. All players will see the scene with this time of day, and the skybox will not follow the day/night cycle.

In the Creator Hub, open the scene settings and click on the **Settings** tab to find the **Skybox** section. Uncheck the **Auto** option and set the time of day you want.

![](/docs/creator/images/fixed-time-of-day.webp)

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

The `fixedTime` property is a number between 0 and 86400, where 0 is midnight and 86400 is 24:00. Any number higher than 86400 is interpreted also as midnight.

Whenever this component is added, removed, or the `fixedTime` property is changed, the skybox time of day transitions smoothly over a few seconds to this new value. The same happens when the player steps out or into the scene. While the skybox time of day is fixed, the skybox will no longer follow progress in its day/night cycle and players can't change the time of day via the UI.

By default, the transition always happens in the forward direction, but you can change this by setting the `transitionMode` property to `TransitionMode.TM_FORWARD` or `TransitionMode.TM_BACKWARD`.

```ts
import { SkyboxTime, TransitionMode } from '@dcl/sdk/ecs'

function main() {
  SkyboxTime.create(engine.RootEntity, { fixedTime: 36000, transitionMode: TransitionMode.TM_BACKWARD })
}
```

### Pick the shorter transition

Nothing ever snaps: every change sweeps through all the times in between. `TM_FORWARD` always runs forward, wrapping past midnight, so going from 10:00 to 08:40 sweeps almost a whole day of sky instead of going back 80 minutes.

Work out which way is shorter and pass the matching mode:

```ts
import { engine, SkyboxTime, TransitionMode } from '@dcl/sdk/ecs'

const DAY = 86400

// Move to a new time of day the short way round
function setTimeOfDay(fixedTime: number) {
  const current = SkyboxTime.getOrNull(engine.RootEntity)?.fixedTime
  // with no SkyboxTime set yet the scene doesn't know the current time, so just go forward
  let transitionMode = TransitionMode.TM_FORWARD
  if (current !== undefined) {
    const forward = (fixedTime - current + DAY) % DAY
    // more than half a day forward means backward is the shorter way
    if (forward > DAY / 2) transitionMode = TransitionMode.TM_BACKWARD
  }
  SkyboxTime.createOrReplace(engine.RootEntity, { fixedTime, transitionMode })
}
```

For a large jump that you don't want the player to watch, hide it behind a full-screen UI: fade in, change the time, wait, then fade out.

## Custom sky, sun and fog

Three components let your scene take over the environment while the player is standing inside it:

* `Skybox`: replaces the sky with your own texture, or recolors the procedural sky, its clouds and its stars.
* `Sun`: recolors, dims, hides or fixes the direction of the sun (and moon) light.
* `EnvironmentFog`: recolors the distance fog and changes its density.

Like `SkyboxTime`, all three can only be added to the root entity of the scene, `engine.RootEntity`. On any other entity they have no effect. Every field is optional: anything you leave out keeps its default time-of-day behavior.


> [!WARNING]
> **📔 Note**: The `Skybox`, `Sun` and `EnvironmentFog` components are not supported yet on the mobile app or on the Bevy explorer. Players on those clients see the regular sky, sun and fog, so make sure your scene still works without these overrides.


```ts
import { engine, Skybox, Sun, EnvironmentFog } from '@dcl/sdk/ecs'
import { Color4, Vector3 } from '@dcl/sdk/math'

function main() {
  Skybox.create(engine.RootEntity, {
    skyboxTexture: 'images/sky.png'
  })
  Sun.create(engine.RootEntity, { intensity: 0.5, direction: Vector3.create(-1, -1, 0) })
  EnvironmentFog.create(engine.RootEntity, {
    color: { keys: [{ time: 0, color: Color4.create(0.6, 0.2, 0.1, 1) }] },
    density: 0.01
  })
}
```

### Scope and reset

The `Skybox`, `Sun` and `EnvironmentFog` overrides apply only while the player is inside your scene. When the player leaves the scene, when a component is removed, or when a field is unset, the sky and lighting go back to their defaults, and re-entering the scene applies them again. Changes apply immediately, without a transition.


> [!WARNING]
> **📔 Note**: While active, these overrides are global: neighboring parcels seen from inside your scene are also rendered with your sky, sun and fog.


### Custom sky texture

`skyboxTexture` replaces the visible sky with an image from the scene's assets. Set it to the path of the image file, the same way as the `src` of a texture. The image must be an equirectangular panorama (a 2:1 latitude-longitude image): its horizontal center faces the positive Z axis (north) of the scene, its top row is the zenith and its bottom row is the nadir. Cube cross, 3x2 and six-face cubemap layouts are not supported.

```ts
import { engine, Skybox } from '@dcl/sdk/ecs'

function main() {
  Skybox.create(engine.RootEntity, {
    skyboxTexture: 'images/sky.png'
  })
}
```

The image wraps around horizontally and is filtered bilinearly. If the image fails to load, the default sky stays in place.

Keep the image size reasonable: the whole panorama is downloaded and kept in memory while your scene is loaded. 2048x1024 pixels is enough for most skies; don't go above 4096x2048. Use a compressed format such as `.jpg` for photographic skies.

The texture only changes what the sky looks like: the sunlight keeps following the time of day, the ambient light keeps following `skyColors` (so you can match it to your image), and the `Sun` and `EnvironmentFog` components still apply on top of it. On the High quality preset, the reflections on shiny materials follow your sky texture too.

### Sky colors, clouds and stars

If you keep the procedural sky, the `Skybox` component lets you recolor it and adjust its cloud layer and star field.

```ts
import { engine, Skybox, ColorGradient } from '@dcl/sdk/ecs'
import { Color4 } from '@dcl/sdk/math'

// A gradient with a single key is a constant color
function constant(color: Color4): ColorGradient {
  return { keys: [{ time: 0, color }] }
}

function main() {
  Skybox.create(engine.RootEntity, {
    skyColors: {
      zenith: constant(Color4.create(0.55, 0.25, 0.12, 1)),
      horizon: constant(Color4.create(0.95, 0.55, 0.3, 1)),
      nadir: constant(Color4.create(0.35, 0.15, 0.08, 1))
    },
    clouds: { opacity: 0.3, speed: 0.01, color: constant(Color4.create(0.9, 0.6, 0.4, 1)) },
    stars: { brightness: 4.62 }
  })
}
```

* `skyColors`: the color of the sky at its `zenith` (straight up), at the `horizon`, and at its `nadir` (the sky below the horizon, visible wherever no terrain covers it, for example at high altitude or in a World with the [landscape terrain](/docs/creator/sdk7/projects/scene-metadata/#landscape-terrain) turned off). These colors also drive the ambient light of the scene: the zenith color lights objects from above, the horizon color from the sides and the nadir color from below, so that objects and avatars match the sky around them. `skyColors.rim` is a narrow glow drawn right on the horizon line, like a sunset glow; when you leave it unset it follows your `horizon` color if you set one (otherwise it keeps its default), so you only need it for an accent (for example an orange sunrise rim on a dark sky).
* `clouds.opacity`: from 0 (no clouds) to 1, the default. `clouds.speed`: how fast the cloud layer turns around the sky, in radians per second: 0.01 by default (one turn every ~10 minutes), 0 for static clouds; negative values count as 0. `clouds.color`: the tint of the cloud layer; without it clouds keep their default time-of-day colors even on a recolored sky.
* `stars.brightness`: a multiplier on the star texture, 4.62 by default (values above 1 are HDR, 0 hides the stars, negative values count as 0). Stars are only visible during the night part of the day.

While a `skyboxTexture` is set, the texture replaces the procedural sky: `clouds` and `stars` have no effect, and `skyColors` no longer color the sky but still drive the ambient light.

### Sun

The `Sun` component changes the directional light of the sun and the moon.

```ts
import { engine, Sun } from '@dcl/sdk/ecs'
import { Color4, Vector3 } from '@dcl/sdk/math'

function main() {
  Sun.create(engine.RootEntity, {
    color: { keys: [{ time: 0, color: Color4.create(1, 0.65, 0.4, 1) }] },
    intensity: 0.5,
    direction: Vector3.create(-1, -1, 0)
  })
}
```

* `color`: the color of the directional light. It also tints the sun disc.
* `visible`: `true` by default. Set it to `false` to hide the sun and moon discs and the sun's lens flare, for example when they would show through a `skyboxTexture`. The light that the sun casts is not affected.
* `intensity`: a multiplier on the time-of-day light intensity, 1 by default. 0 turns the sunlight off; negative values count as 0.
* `direction`: the direction in which the sunlight travels, in scene coordinates: `(0, -1, 0)` puts the sun straight overhead, and `(-1, -1, 0)` puts it in the positive X (east) side of the sky, 45 degrees above the horizon. While it's set, the sun stays fixed at every time of day, but its color and intensity still follow the time of day. Leave it unset, or set it to `(0, 0, 0)`, to keep the regular path of the sun across the sky.

### Fog

The `EnvironmentFog` component changes the distance fog.

```ts
import { engine, EnvironmentFog } from '@dcl/sdk/ecs'
import { Color4 } from '@dcl/sdk/math'

function main() {
  EnvironmentFog.create(engine.RootEntity, {
    color: { keys: [{ time: 0, color: Color4.create(0.85, 0.5, 0.3, 1) }] },
    density: 0.01
  })
}
```

* `color`: the color of the fog.
* `density`: how quickly things fade into the fog, as an exponential density per meter: the fog at a given distance is `1 - e^(-density * distance)`, so `1 / density` is roughly the distance at which two thirds of the view is fogged. The default is 0.0005 (about 2 km); 0.02 fogs about two thirds of the view at 50 m and almost all of it by 150 m, and 0 makes the fog invisible. Negative values count as 0.

Whether fog is rendered at all is a quality setting chosen by the player; a scene can't turn it on or off.

### Color gradients over the day

Every color in these components is a `ColorGradient`: a list of `keys`, each with a `time` and a `Color4`. The `time` is the normalized time of day, from 0 (_00:00_) to 1 (_24:00_), so 0.5 is noon. It's the `SkyboxTime` clock divided by 86400: `fixedTime: 21600` (6 AM) is `time: 0.25`. The color is interpolated between neighboring keys; before the first key and after the last one, that key's color is used. Times outside 0 to 1 are clamped into that range, and if two keys share the same `time`, the later one in the list wins. A gradient with a single key is simply a constant color.

```ts
import { engine, Skybox } from '@dcl/sdk/ecs'
import { Color4 } from '@dcl/sdk/math'

function main() {
  Skybox.createOrReplace(engine.RootEntity, {
    skyColors: {
      horizon: {
        keys: [
          { time: 0, color: Color4.create(0.05, 0.05, 0.2, 1) }, // midnight
          { time: 0.25, color: Color4.create(0.95, 0.5, 0.4, 1) }, // dawn
          { time: 0.5, color: Color4.create(0.7, 0.9, 1, 1) }, // noon
          { time: 0.75, color: Color4.create(0.95, 0.4, 0.2, 1) }, // dusk
          { time: 1, color: Color4.create(0.05, 0.05, 0.2, 1) } // midnight again, so there's no jump
        ]
      }
    }
  })
}
```

Gradients don't wrap around midnight, so repeat the same color at `time: 0` and `time: 1` if the day should loop seamlessly. Color values are not limited to 1: values above 1 produce a brighter, HDR sun or sky. The alpha channel is ignored. Use up to 8 keys per gradient: longer gradients are resampled to 8 evenly spaced keys, which smooths out any detail between them. Combine gradients with `SkyboxTime` to pin the day at one specific point of your gradient.

### Complete darkness

To make the lights placed in your scene the only source of illumination, black out everything the environment contributes: the sunlight and its disc, the sky colors (and with them the ambient light), the fog, the clouds and the stars.

```ts
import { engine, Skybox, Sun, EnvironmentFog } from '@dcl/sdk/ecs'
import { Color4 } from '@dcl/sdk/math'

function main() {
  const black = { keys: [{ time: 0, color: Color4.Black() }] }

  Skybox.createOrReplace(engine.RootEntity, {
    skyColors: { zenith: black, horizon: black, nadir: black },
    clouds: { opacity: 0 },
    stars: { brightness: 0 }
  })
  Sun.createOrReplace(engine.RootEntity, { intensity: 0, visible: false })
  EnvironmentFog.createOrReplace(engine.RootEntity, { color: black })
}
```


> [!WARNING]
> **📔 Note**: On the Low quality preset, scene lights are switched off, so a fully dark scene renders completely black for those players. Use emissive materials to keep anything essential visible.


See [Lights](/docs/creator/sdk7/3d-essentials/lights/) to add point and spot lights to your scene.


> [!NOTE]
> **💡 Tip**: For working examples of skybox control, see the [`2,1-skybox-sdk-scene-a`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/2,1-skybox-sdk-scene-a) and [`3,1-skybox-sdk-scene-b`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/3,1-skybox-sdk-scene-b) test scenes, which drive `SkyboxTime` and `TransitionMode` live from a UI panel, and [`2,0-skybox-scene-json`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/2,0-skybox-scene-json), which reads the `fixedTime` set in `scene.json` back at runtime via `getSceneInformation()`. The [`2,2-skybox-control`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/2,2-skybox-control) scene exercises the `Skybox`, `Sun` and `EnvironmentFog` components: sky textures, environment presets, hiding the sun and a complete-darkness mode.

