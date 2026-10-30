> Change the player's speed and jump height, or restrict how they can move

# Locomotion Settings and Restrictions

You can change how the player's avatar moves while inside your scene. Use the `AvatarLocomotionSettings` component to tune speeds and jump heights, and the `InputModifier` component to freeze the player or block specific kinds of movement. Both components go on `engine.PlayerEntity`, and both only take effect while the player is inside your scene's bounds.

## Locomotion Settings

You can affect the player's locomotion, like their running speed, jump height, and more. This can be altered dynamically, for example to allow a player to collect a temporary speed boost by interacting with an item, or to disable the player's ability to jump for a short period of time.


> [!NOTE]
> **💡 Tip**: You can set these values without writing code. Drag the **Locomotion Settings** smart item into your scene in the [Scene Editor in Creator Hub](/docs/creator/scene-editor/get-started/about-editor/) and move its sliders. See [Locomotion settings](/docs/creator/scene-editor/interactivity/smart-items/#locomotion-settings).


To do this, add an `AvatarLocomotionSettings` component to the `engine.PlayerEntity`.

```ts
import {AvatarLocomotionSettings, engine} from '@dcl/sdk/ecs'

AvatarLocomotionSettings.create(engine.PlayerEntity, {
	runSpeed: 10,
	jumpHeight: 2,
})
```

The following properties are available:

- `walkSpeed`: The speed at which the player walks, in meters per second. On the desktop client, players walk by pressing the control key.
- `jogSpeed`: The speed at which the player jogs, in meters per second. This is the default way in which the player moves.
- `runSpeed`: The speed at which the player runs, in meters per second. On the desktop client, players run by pressing the shift key.
- `jumpHeight`: The height at which the player jumps, in meters.
- `runJumpHeight`: The height at which the player jumps after running, in meters.
- `doubleJumpHeight`: The height of the second jump when double-jumping, in meters.
- `glidingSpeed`: The horizontal speed at which the player moves while gliding, in meters per second.
- `glidingFallingSpeed`: The maximum falling speed of the player while gliding, in meters per second. This only caps the player's descent: upward motion, like a lift from a scene's continuous force, isn't limited.
- `hardLandingCooldown`: The cooldown after a hard landing, in seconds. This is the time that the player has to wait before they can move again after landing from a high fall.

For reference, here are the default values for those properties:

- `walkSpeed`: 1.5 m/s
- `jogSpeed`: 8 m/s
- `runSpeed`: 10 m/s
- `glidingSpeed`: 6 m/s
- `glidingFallingSpeed`: 1 m/s
- `jumpHeight`: 1 m
- `runJumpHeight`: 1.5 m
- `doubleJumpHeight`: 2 m
- `hardLandingCooldown`: 0.75 s


> [!NOTE]
> **💡 Tip**: While gliding, continuous forces applied by the scene are 1.5 times stronger, and upward forces can lift the player. See [Forces while gliding](/docs/creator/sdk7/interactivity/player-physics/#forces-while-gliding).



> [!NOTE]
> **💡 Tip**: None of these properties can be lower than 0. If you set one of them to a negative value, it will be clamped to 0. Setting these values to zero will have the same effect as using the `InputModifier` to block the use of certain keys.
> 
> You can only affect the player's locomotion if they are inside the scene's bounds. To affect other players' avatars, you must run the code that affects their locomotion on their own instance.


You can create a [smart wearable](/docs/creator/sdk7/projects/smart-wearables/) that makes the player always run faster or jump higher. If both the scene and a smart wearable define different values for these parameters, the scene values are always used.

To ensure nobody has unfair advantages at a parkour scene, you can enforce the default parameters by explicitly adding their default values in your scene:

```ts
import {AvatarLocomotionSettings, engine} from '@dcl/sdk/ecs'

AvatarLocomotionSettings.create(engine.PlayerEntity, {
	runSpeed: 10,
	walkSpeed: 1.5,
	jogSpeed: 8 ,
	jumpHeight: 1,
	runJumpHeight: 1.5,
	hardLandingCooldown: 0.75
})
```

## Restrict locomotion

You can restrict what actions the player can perform in your scene. Use this to freeze the player, or to restrict specific forms of locomotion, for example to prevent the player from jumping or running.

### Freeze the player

You can freeze the player so that none of the input keys can move the avatar. This can be useful for many game mechanics. It's also a good practice to freeze a player while performing an important animation that shouldn't be interrupted by movement, or while a [Virtual Camera](/docs/creator/sdk7/3d-essentials/camera/) points away from the avatar and you don't want the player to move blindly.

Use the `InputModifier` component on the `engine.PlayerEntity` to prevent the player's inputs from affecting the avatar's locomotion. The avatar will remain still, the player will only be able to rotate the camera.

```ts
import {InputModifier, engine} from '@dcl/sdk/ecs'

InputModifier.create(engine.PlayerEntity, {
	mode: InputModifier.Mode.Standard({
		disableAll: true,
	}),
})
```

Keep the following considerations in mind:

* While the player's interactions are disabled, their avatar is still affected by external forces, like gravity or moving platforms.
* The `InputModifier` component can only be used with the `engine.PlayerEntity` entity. It can only affect the current player, it can't affect other players.
* This component only affects the player while the avatar is within your scene's bounds. Their locomotion stops being restricted as soon as they leave the scene.
* While the player's interactions are disabled, the player can't perform emotes freely, but the scene can trigger animations on the avatar.
* Player inputs don't affect the avatar, but the [global input events](/docs/creator/sdk7/interactivity/button-events/system-based-events/#global-input-events) can still be listened to by the scene. You could use these to control a vehicle, or use a [Virtual Camera](/docs/creator/sdk7/3d-essentials/camera/) to follow another entity as it moves, treating it as an alternative avatar.

### Restricting specific kinds of locomotion

Instead of entirely freezing the player, you can restrict certain specific forms of locomotion of the player. This could be used for gameplay reasons, for example to preserve the difficulty of a platformer by preventing double-jump and glide. These abilities could even be toggled dynamically as a game mechanic, for example giving the player a stamina bar and preventing them from running when it's depleted. It could also be used to set the tone of a scene, for example preventing running or jumping in a location that is meant to be serene. The `InputModifier` includes the following options:

* `disableWalk`: Player can't walk slowly (pressing control). If the player tries to walk, they will jog or run instead, if allowed.
* `disableRun`: Player can't run (pressing shift). If the player tries to run, they will jog instead, if allowed.
* `disableJog`: Player can't jog (this is the default movement speed). If the player tries to jog, they will run or walk instead, if allowed.
* `disableJump`: Player can't jump.
* `disableEmote`: Player can't perform emotes voluntarily. The scene is able to trigger animations on the player's avatar.
* `disableDoubleJump`: The player can't perform a double-jump.
* `disableGliding`: The player can't glide.

```ts
import {InputModifier, engine} from '@dcl/sdk/ecs'

InputModifier.create(engine.PlayerEntity, {
	mode: InputModifier.Mode.Standard({
		disableAll: false,
		disableWalk: false,
		disableRun: true,
		disableJog: true,
		disableJump: true,
		disableEmote: true,
		disableDoubleJump: true,
		disableGliding: true
	}),
})
```

### Advanced syntax

To use the component without any helpers, you can use the following syntax:

```ts
import {InputModifier, engine} from '@dcl/sdk/ecs'

InputModifier.createOrReplace(engine.PlayerEntity, {
	mode: {
		$case: 'standard',
		standard: {
			disableAll: false,
			disableWalk: false,
			disableRun: true,
			disableJog: true,
			disableJump: true,
			disableEmote: true,
		},
	},
})
```


> [!NOTE]
> **💡 Tip**: For a working example, see the [`0,1-input-modifier`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/0,1-input-modifier) test scene, which toggles each `InputModifier` movement flag separately and shows an avatar still being carried by a moving platform while movement is blocked.

