> Add particle effects like fire, rain, sparks, and magic auras to your scene

# Particle Systems

Particle systems let you create dynamic visual effects by emitting and animating large numbers of small sprites. Use them to build fire, smoke, rain, snow, sparks, magic auras, explosions, and many other effects that would be impractical to achieve with static meshes.

## Adding a particle system

To add a particle system to your scene, create an entity and attach the `ParticleSystem` component to it.

```ts
import { engine, Transform, ParticleSystem } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

const emitter = engine.addEntity()

Transform.create(emitter, {
	position: Vector3.create(8, 1, 8),
})

ParticleSystem.create(emitter, {})
```

With no properties set, the component uses all defaults: a point emitter shooting 10 particles per second, with a 5-second particle lifetime.

## Emitter shapes

The emitter shape determines where particles spawn. Four shapes are available via the `ParticleSystem.Shape` helpers.

### Point

Particles spawn from a single point (the entity's position). This is the default when no shape is specified.

```ts
ParticleSystem.create(emitter, {
	shape: ParticleSystem.Shape.Point(),
})
```

### Sphere

Particles spawn from the surface or interior of a sphere.

```ts
ParticleSystem.create(emitter, {
	shape: ParticleSystem.Shape.Sphere({ radius: 2 }),
})
```

### Cone

Particles spawn from the base of a cone and travel outward in the cone's direction. `angle` is the half-angle in degrees; `radius` is the base radius in meters. To orient the direction of the cone, set the rotation of the entity's `Transform`.

```ts
ParticleSystem.create(emitter, {
	shape: ParticleSystem.Shape.Cone({ angle: 25, radius: 1 }),
})
```

### Box

Particles spawn from anywhere inside a box volume.

```ts
ParticleSystem.create(emitter, {
	shape: ParticleSystem.Shape.Box({ size: Vector3.create(3, 1, 3) }),
})
```

## Emission properties

Control how many particles are created and how long they live.

| Property       | Default | Description                                            |
| -------------- | ------- | ------------------------------------------------------ |
| `rate`         | `10`    | Particles emitted per second (continuous).             |
| `maxParticles` | `1000`  | Hard cap on simultaneous live particles.               |
| `lifetime`     | `5`     | Each particle's lifespan in seconds.                   |
| `active`       | `true`  | Whether the system is actively emitting new particles. |

```ts
ParticleSystem.create(emitter, {
	rate: 50,
	maxParticles: 500,
	lifetime: 3,
})
```

## Motion

### Gravity

The `gravity` property is a multiplier applied to the scene's gravity (-9.81 m/s²). Set it to `0` to make particles float in place, a negative value to push them upward, or a positive value to accelerate them downward.

```ts
ParticleSystem.create(emitter, {
	gravity: -0.5, // particles float slowly upward
})
```

### Initial velocity

`initialVelocitySpeed` is a `FloatRange` that sets a min and max for the random speed range that each particle launches at (in m/s). Both values default to `1`.

```ts
ParticleSystem.create(emitter, {
	initialVelocitySpeed: { start: 2, end: 8 },
})
```

### Additional force

A constant force vector applied to all particles every frame, in addition to gravity. Useful for wind or magnetic effects.

```ts
ParticleSystem.create(emitter, {
	additionalForce: Vector3.create(0.5, 0, 0), // constant sideways push
})
```

### Velocity limiting

Use `limitVelocity` to cap the maximum speed of particles and optionally dampen their excess velocity each frame.

```ts
ParticleSystem.create(emitter, {
	limitVelocity: {
		speed: 5, // maximum allowed speed in m/s
		dampen: 0.8, // fraction of excess velocity removed per frame
	},
})
```

## Visual properties

### Color

`initialColor` sets the particle's color at birth, picked randomly within the given range. `colorOverTime` makes the particle's color transition gradually from the `start` to the `end` value over the particle's lifetime. Both accept a `ColorRange` with a `start` and `end` value.

```ts
import { Color4 } from '@dcl/sdk/math'

ParticleSystem.create(emitter, {
	initialColor: {
		start: Color4.create(1, 0.5, 0, 1), // orange
		end: Color4.create(1, 1, 0, 1), // yellow
	},
	colorOverTime: {
		start: Color4.create(0.5, 0, 0, 0.5), // dim red, half transparent
		end: Color4.create(0, 0, 0, 0), // fully transparent (fade out)
	},
})
```

Note that the 4th value in a `Color4` is the _alpha_. If you set the final color to one with an alpha of 0, the particles will gradually fade out and become invisible, which is often a good effect.

### Size

`initialSize` and `sizeOverTime` are `FloatRange` values. `initialSize` controls the particle's scale at birth, picked randomly within the given range. `sizeOverTime` makes the particle's scale transition gradually from the `start` to the `end` value over the particle's lifetime. A value of `1` equals the original texture size.

```ts
ParticleSystem.create(emitter, {
	initialSize: { start: 0.1, end: 0.3 },
	sizeOverTime: { start: 0.5, end: 1.0 }, // grow over lifetime
})
```


> [!WARNING]
> **📔 Note**: The `scale` of the entity's `Transform` doesn't affect the scale of the particles.


### Texture

By default particles render as white squares. Supply a texture to use a custom image.

```ts
ParticleSystem.create(emitter, {
	texture: { src: 'assets/scene/Images/spark.png' },
})
```

### Blend mode

Controls how the particle color is composited with the scene behind it.

| Value                                  | Description                                                                |
| -------------------------------------- | -------------------------------------------------------------------------- |
| `ParticleSystemBlendMode.PSB_ALPHA`    | Standard transparency (default).                                           |
| `ParticleSystemBlendMode.PSB_ADD`      | Additive blending — particles brighten the scene. Good for fire and glows. |
| `ParticleSystemBlendMode.PSB_MULTIPLY` | Multiplies particle color with the scene behind it.                        |

```ts
import { ParticleSystemBlendMode } from '@dcl/sdk/ecs'

ParticleSystem.create(emitter, {
	texture: { src: 'assets/scene/textures/ember.png' },
	blendMode: ParticleSystemBlendMode.PSB_ADD,
})
```

### Billboard

When `billboard` is `true` (the default), each particle always faces the camera regardless of the emitter's orientation. Set it to `false` for particles that should tumble in 3D space.

```ts
ParticleSystem.create(emitter, {
	billboard: false,
})
```

## Rotation

### Initial rotation and rotation over time

`initialRotation` is the orientation of a particle when it spawns. `rotationOverTime` is a per-axis angular velocity applied each second. Both accept a `Quaternion`.

```ts
import { Quaternion } from '@dcl/sdk/math'

ParticleSystem.create(emitter, {
	initialRotation: Quaternion.fromEulerDegrees(0, 0, 45),
	rotationOverTime: Quaternion.fromEulerDegrees(0, 0, 90), // spin 90°/s on Z
})
```


> [!WARNING]
> **📔 Note**: If `billboard` is set to `true`, then particles will only rotate in one axis, always maintaining their camera-facing orientation.


### Face travel direction

When `faceTravelDirection` is `true`, each particle automatically rotates to point in its direction of movement, like an asteroid or leaf falling through the air.

```ts
ParticleSystem.create(emitter, {
	faceTravelDirection: true,
	billboard: false,
})
```


> [!WARNING]
> **📔 Note**: If `faceTravelDirection` is true, the value of `billboard` is ignored.


## Sprite sheet animation

You can animate particles by treating a texture as a grid of animation frames. Specify the number of columns and rows in the sheet and the playback speed.

```ts
ParticleSystem.create(emitter, {
	texture: { src: 'assets/scene/textures/flame-sheet.png' },
	spriteSheet: {
		tilesX: 4, // 4 columns
		tilesY: 3, // 3 rows  (12 frames total)
		framesPerSecond: 12,
	},
})
```

## Playback control

The particle system can be in one of three states controlled by `playbackState`.

| Value                                    | Description                                                |
| ---------------------------------------- | ---------------------------------------------------------- |
| `ParticleSystemPlaybackState.PS_PLAYING` | Actively emitting (default).                               |
| `ParticleSystemPlaybackState.PS_PAUSED`  | Freezes all current particles in place and stops emitting. |
| `ParticleSystemPlaybackState.PS_STOPPED` | Stops emitting and removes all existing particles.         |

```ts
import { ParticleSystemPlaybackState } from '@dcl/sdk/ecs'

// Pause when player walks away
ParticleSystem.getMutable(emitter).playbackState =
	ParticleSystemPlaybackState.PS_PAUSED

// Resume
ParticleSystem.getMutable(emitter).playbackState =
	ParticleSystemPlaybackState.PS_PLAYING
```

### Loop and prewarm

By default the system loops indefinitely. Set `loop` to `false` for a one-shot effect that stops automatically after all particles have died.

Set `prewarm` to `true` (requires `loop: true`) to simulate the system as if it had been running from the start, so particles already fill the scene when the player first sees it.

```ts
ParticleSystem.create(emitter, {
	loop: false, // play once then stop
	prewarm: false,
})
```

## Burst emission

Bursts let you emit a large number of particles at specific moments rather than at a constant rate. They are useful for explosions, fireworks, or other one-shot events.

A single particle system can go through a cycle with several bursts, even with varying intervals or probabilities so as to feel more natural.

```ts
ParticleSystem.create(emitter, {
	rate: 0, // disable continuous emission
	bursts: {
		values: [
			{
				time: 0, // time in seconds after playback starts
				count: 200, // particles to emit per burst
				cycles: 1, // how many times to repeat (0 = infinite)
				interval: 0.2, // seconds between repeated cycles
				probability: 1, // 0–1 chance the burst fires each cycle
			},
		],
	},
})
```

For a looping fireworks effect with staggered bursts:

```ts
ParticleSystem.create(emitter, {
	loop: true,
	rate: 0,
	lifetime: 2,
	bursts: {
		values: [
			{ time: 0.0, count: 80, cycles: 0, interval: 3 },
			{ time: 0.7, count: 100, cycles: 0, interval: 3 },
			{ time: 1.4, count: 60, cycles: 0, interval: 3 },
		],
	},
})
```

## Simulation space

Controls whether particles move relative to the emitter (`PSS_LOCAL`, default) or remain fixed in world coordinates after spawning (`PSS_WORLD`).

Use `PSS_WORLD` for a moving emitter that should leave a trail behind it (e.g., an asteroid or a rocket).

```ts
import { PBParticleSystem_SimulationSpace } from '@dcl/sdk/ecs'

ParticleSystem.create(emitter, {
	simulationSpace: PBParticleSystem_SimulationSpace.PSS_WORLD,
})
```

## Performance

The engine enforces a per-scene particle budget and will automatically scale down emission rates across all active particle systems in the scene if the total particle count would exceed the limit. Plan your scenes accordingly:

- Prefer fewer, visually impactful systems over many low-impact ones.
- Use `maxParticles` to cap individual systems.
- Use `active` or `playbackState` to disable systems that are off-screen or out of range.
- Short `lifetime` values keep the live-particle count lower than a high emission `rate` alone suggests.

The engine caps the number of maximum particles that are rendered at any given time to 1000. If you're emitting more particles than that, you may not be seeing all of them.

Also keep in mind that particles can only be seen by a player while they're standing inside your scene. Players who are looking at the scene from the outside will not see any particles until they step in.

## Particle Lab

Enter the world [ParticleLab.dcl.eth](decentraland://?realm=particlelab.dcl.eth&dclenv=org) to experiment with different particle systems. When you approach a particle system, the UI will display all the available fields, you can tweak them live without needing to reload the scene. When you're happy, click the **Copy** button to copy the code for that particle system to your clipboard.


> [!NOTE]
> **💡 Tip**: For a working example that exercises every `ParticleSystem` field, see the [`0,7-particle-system`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/0,7-particle-system) test scene. It covers all four emitter shapes, the `PSB_ADD`/`PSB_ALPHA`/`PSB_MULTIPLY` blend modes, the `PS_PLAYING`/`PS_PAUSED`/`PS_STOPPED` playback states, sprite sheets, continuous `rate` against one-shot and looping `bursts`, and `PSS_LOCAL` against `PSS_WORLD` on a moving emitter.

