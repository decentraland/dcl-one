> Make the player's avatar play default or custom animations, and detect when an emote finishes

# Avatar Animations

You can make the player perform an animation as part of the scene's code. This can help provide more immersion, and it can also help communicate what other players are doing to each other. The avatar animations are seen both by the player (in 3rd person view) and any other players around.

Player-controlled animations that run on the full body are overridden by the default locomotion animations, like walking and jumping. So full-body animations played by the scene only play while the player is standing still. If the player walks or jumps, any full-body animations are interrupted. Upper-body animations don't get interrupted by movement. 


> [!WARNING]
> **📔 Note**: Players can only be animated if they already are standing inside the scene's bounds, not if they are on a neighboring scene. Smart wearables can play animations anywhere.
> 
> While a player is performing an animation, they are not affected by collisions, and their movements aren't constrained by the scene's physics. Also note that if an animation displaces the player from their original position (for example, if the animation involves a jump), the player's Transform component will not be affected by this displacement.


### Use the Scene Editor

The easiest way to make a player perform an animation is to use the Scene Editor. Use the no-code **Play Emote** action to play a default animation, or the **Play Custom Emote** action to play an animation from a file. See [Make any item smart](/docs/creator/scene-editor/interactivity/make-any-item-smart/).

### Default animations

Use the `triggerEmote()` function to run one of the default animations that players are able to play anywhere in Decentraland. This function takes an object with the following properties as an argument:

* `predefinedEmote`: A string name for an existing emote.
* `mask`: (optional) Play the animation on only part of the avatar's body, using a value from the `AvatarMask` enum. For example, `AvatarMask.AM_UPPER_BODY` animates only the avatar's upper body. See [Animate only the upper body](#animate-only-the-upper-body).

```ts
import { triggerEmote } from '~system/RestrictedActions'

const emoter = engine.addEntity()
Transform.create(emoter, { position: Vector3.create(8, 0, 8) })
MeshRenderer.setBox(emoter)
MeshCollider.setBox(emoter)
pointerEventsSystem.onPointerDown(
	{
		entity: emoter,
		opts: { button: InputAction.IA_POINTER, hoverText: 'Dance' },
	},
	() => {
		triggerEmote({ predefinedEmote: 'robot' })
	}
)
```

The following emotes show feedback about player actions in your scene, all of these are valid values for the `predefinedEmote` field:

* `buttonDown`
* `buttonFront`
* `getHit`
* `knockOut`
* `lever`
* `openChest`
* `openDoor`
* `punch`
* `push`
* `swingWeaponOneHand`
* `swingWeaponTwoHands`
* `throw`
* `sittingChair1`
* `sittingChair2`
* `sittingGround1`
* `sittingGround2`

These emotes are available to all players in their default emote wheel, and can also be used in any scene.

* `wave`
* `fistpump`
* `robot`
* `raiseHand`
* `clap`
* `money`
* `kiss`
* `tik`
* `hammer`
* `tektonik`
* `dontsee`
* `handsair`
* `shrug`
* `disco`
* `dab`
* `headexplode`


> [!NOTE]
> **💡 Tip**: If a player walks or jumps while playing the animation, they will interrupt it. If you don't want that to be possible, you can freeze the avatar with [Input Modifiers](/docs/creator/sdk7/interactivity/avatars/locomotion/#freeze-the-player) for the duration of the avatar animation.


### Custom animations

Use the `triggerSceneEmote()` to make the player perform a custom animation, stored as a .glb file as part of the scene's asset.


> [!WARNING]
> **📔 Note**: The file's name **must** end in `_emote.glb` to work as an avatar animation.


This function takes an object with the following properties:

* `src`: A string with a path to the emote file.
* `loop`: If true, the animation will loop continuously until the player moves or the animation is stopped. False by default.
* `mask`: (optional) Play the animation on only part of the avatar's body, using a value from the `AvatarMask` enum. For example, `AvatarMask.AM_UPPER_BODY` animates only the avatar's upper body. See [Animate only the upper body](#animate-only-the-upper-body).

```ts
import { triggerSceneEmote } from '~system/RestrictedActions'
import { AvatarMask } from '@dcl/sdk/ecs'

const emoter = engine.addEntity()
Transform.create(emoter, { position: Vector3.create(8, 0, 8) })
MeshRenderer.setBox(emoter)
MeshCollider.setBox(emoter)
pointerEventsSystem.onPointerDown(
	{
		entity: emoter,
		opts: { button: InputAction.IA_POINTER, hoverText: 'Make snowball' },
	},
	() => {
		triggerSceneEmote({ src: 'animations/Snowball_Throw_emote.glb', loop: false, mask: AvatarMask.AM_UPPER_BODY })
	}
)
```


> [!NOTE]
> **💡 Tip**: If a player walks or jumps while playing the animation, they will interrupt it. If you don't want that to be possible, you can freeze the avatar with [Input Modifiers](/docs/creator/sdk7/interactivity/avatars/locomotion/#freeze-the-player) for the duration of the avatar animation.


### Animate only the upper body

Both `triggerEmote()` and `triggerSceneEmote()` accept an optional `mask` property, that limits the animation to only part of the avatar's body.

Normally, an animation stops as soon as the player walks or jumps, because the default locomotion animations take over the whole body. When you set `mask` to `AvatarMask.AM_UPPER_BODY`, the animation only drives the avatar from the waist up, and the legs remain controlled by the default locomotion animations. This means the player can keep walking or running around the scene while the upper body plays your animation. Use this for actions that shouldn't interrupt movement, like carrying a crate, holding a torch, or juggling.

```ts
import { triggerSceneEmote } from '~system/RestrictedActions'
import {
	engine,
	AvatarMask,
	InputAction,
	MeshCollider,
	MeshRenderer,
	pointerEventsSystem,
	Transform,
} from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

const cheerButton = engine.addEntity()
Transform.create(cheerButton, { position: Vector3.create(8, 0, 8) })
MeshRenderer.setBox(cheerButton)
MeshCollider.setBox(cheerButton)
pointerEventsSystem.onPointerDown(
	{
		entity: cheerButton,
		opts: { button: InputAction.IA_POINTER, hoverText: 'Cheer' },
	},
	() => {
		triggerSceneEmote({
			src: 'animations/Cheer_emote.glb',
			loop: true,
			mask: AvatarMask.AM_UPPER_BODY,
		})
	}
)
```

In this example, the player clicks a button and starts cheering with their arms in the air, in a looping animation. Since only the upper body is animated, they can keep running around the scene while they cheer, for example to follow the action in a race or a football match.

Some things to keep in mind:

* `AvatarMask.AM_UPPER_BODY` is currently the only value in the `AvatarMask` enum. To play an animation on the full body, simply don't set the `mask` property.
* The `loop` property behaves the same as with full-body animations: with `loop: false` the masked animation plays once and the upper body then returns to normal locomotion, with `loop: true` it repeats until stopped.
* To stop a looping masked animation from code, call `stopEmote({})`, also imported from `~system/RestrictedActions`.

### Detect when an emote finishes

Every emote lifecycle event is reported through the `AvatarEmoteCommand` component on the player entity. Each new entry carries a `state` field, with a value from the `EmoteState` enum:

* `EmoteState.ES_STARTED`: The emote started playing. This is also the value reported when the `state` field is absent (entries written by older clients).
* `EmoteState.ES_FINISHED`: A non-looping emote played through to its natural end.
* `EmoteState.ES_INTERRUPTED`: The emote was cut short: the player moved or jumped, teleported, another emote started, the emote was explicitly stopped, or the player left the scene.

This works for emotes triggered by the scene (`triggerEmote()` and `triggerSceneEmote()`) as well as emotes the player plays themselves through the emote wheel, and also for emotes played by other players in the scene.

Use the `onChange` function on the `AvatarEmoteCommand` component to react to each new entry:

```ts
import { AvatarEmoteCommand, EmoteState } from '@dcl/sdk/ecs'

export function main() {
	AvatarEmoteCommand.onChange(engine.PlayerEntity, (emote) => {
		if (!emote) return

		switch (emote.state ?? EmoteState.ES_STARTED) {
			case EmoteState.ES_STARTED:
				console.log('emote started: ', emote.emoteUrn)
				break
			case EmoteState.ES_FINISHED:
				console.log('emote finished naturally: ', emote.emoteUrn)
				break
			case EmoteState.ES_INTERRUPTED:
				console.log('emote was interrupted: ', emote.emoteUrn)
				break
		}
	})
}
```


> [!WARNING]
> **📔 Note**: This feature is only supported in the Desktop client. Emotes played with a `mask` (partial-body emotes) on the local player don't currently report lifecycle events.



> [!NOTE]
> **💡 Tip**: To play animations on avatars that aren't players, see [NPC Avatars](/docs/creator/sdk7/interactivity/avatars/npc-avatars/#animations).

