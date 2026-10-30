> How to animate 3D models in your scene

# 3D Model Animations

3D models in _.glTF_ and _.glb_ format can include as many animations as you want in them. Animations tell the mesh how to move, by specifying a series of _keyframes_ that are laid out over time, the mesh then blends from one pose to the other to simulate continuous movement.

Most 3D model animations are [_skeletal animations_](https://en.wikipedia.org/wiki/Skeletal_animation). These animations simplify the complex geometry of the model into a "stick figure", linking every vertex in the mesh to the closest _bone_ in the _skeleton_. Modelers adjust the skeleton into different poses, and the mesh stretches and bends to follow these movements.

As an alternative, _vertex animations_ animate a model without the need of a skeleton. These animations specify the position of each vertex in the model directly. Decentraland supports these animations as well.

See [Animations](https://github.com/decentraland/docs-creator/blob/main/creator/3d-modeling/animations/README.md) for details on how to create animations for a 3D model. Read [Shape components](/docs/creator/sdk7/3d-essentials/shape-components/) for instructions on how to import a 3D model to a scene.


> [!NOTE]
> **💡 Tip**: Animations are usually better for moving something in place, not for changing the position of an entity. For example, you can set an animation to move a character's feet in place, but to change the location of the entity it's best to use the Transform component. See [Positioning entities](/docs/creator/sdk7/3d-essentials/move-entities/) for more details.


## Check a 3D model for animations

Not all _glTF_ files include animations. To see if there are any available, you can do the following:

* If using [VS Code](https://code.visualstudio.com/)(recommended), install the _GLTF Tools_ extension and view the contents of a glTF file there.
* Open the [Babylon Sandbox](https://sandbox.babylonjs.com/) site and drag the glTF file (and any _.jpg_ or _.bin_ dependencies) to the browser.
* Open the _.glTF_ file with a text editor and scroll down till you find _"animations":_.


> [!NOTE]
> **💡 Tip**: In _skeletal_ animations, an animation name is often comprised of its armature name, an underscore and its animation name. For example `myArmature_animation1`.


## Automatic playing

If a 3D model includes any animations, the default behavior is that the first of these is always played on a loop.

To avoid this behavior, add an `Animator` component to the entity that has the model, and then handle the playing of animations explicitly. If an `Animator` component is present in the entity, all animations default to a `playing: false` state, and need to be manually played.


> [!NOTE]
> **💡 Tip**: In the [Scene Editor](/docs/creator/scene-editor/get-started/about-editor/), you can add an **Animator** component visually. See [Add Components](/docs/creator/scene-editor/build/components/#add-components). You can also control animations in a no-code way via **Actions**, see [Make any item smart](/docs/creator/scene-editor/interactivity/make-any-item-smart/).


## Handle animations explicitly

An `Animator` component is used to access all the animations of the entity and can be used to explicitly tell the entity to play or stop an animation. The `Animator` component includes an array of `states`, this list must include one object for each one of the animations that the 3D model can perform. A single `Animator` can include as many states as needed.

```ts
// Create entity
const shark = engine.addEntity()

// Add a 3D model to it
GltfContainer.create(shark, {
	src: 'models/shark.glb',
})

Animator.create(shark, {
	states: [
		{
			clip: 'swim',
			playing: true,
			loop: true,
		},
	],
})
```

Each `state` object keeps track of if an animation is currently playing.


> [!WARNING]
> **📔 Note**: The `Animator` component must be imported via
> 
> > `import { Animator } from "@dcl/sdk/ecs"`
> 
> See [Imports](/docs/creator/sdk7/getting-started/coding-scenes/#imports) for how to handle these easily.


## Fetch an animation

Fetch a clip from the `Animator` by name using the `.Animator.getClip()` function. This function returns a mutable version of the animation state object.

```ts
const swimAnim = Animator.getClip(sharkEntity, 'swim')
```

`Animator.getClip` requires the following parameters:

* `entity`: The entity of the `Animator` component that you want to query.
* `clipName`: String for the name of the clip you want to fetch.

`Animator.getClip` fetches a mutable version of the animation state, so you can modify values freely on what this function returns.

```ts
const swimAnim = Animator.getClip(sharkEntity, 'swim')
swimAnim.loop = false
```


> [!WARNING]
> **📔 Note**: If you attempt to use `Animator.getClip()` to fetch a clip that is not listed in the `Animator` component, it throws an error. Use `Animator.getClipOrNull()` if you prefer to get a `null` response in that case, instead of an error.


## Play an animation

The `.playing` field in an animation state determines if the animation is currently playing. Note that multiple animations may be playing in a single 3D model at the same time.

Use the `Animator.playSingleAnimation()` function on an `AnimationState` object.

```ts
Animator.playSingleAnimation(sharkEntity, 'swim')
```

If the entity was playing any other animations, `Animator.playSingleAnimation` stops them.

`Animator.playSingleAnimation` requires the following parameters:

* `entity`: The entity of the `Animator` component that you want to affect.
* `clipName`: String for the name of the clip you want to play.
* `resetCursor`: _(optional)_ If _true_, it plays the animation from the start, even if the animation was previously paused. If _false_, it will keep playing the animation from where it was paused. Default: _true_.

```ts
Animator.playSingleAnimation(sharkEntity, 'swim', false)
```

The following table summarizes how `Animator.playSingleAnimation()` behaves, using different values for the `resetCursor` property:

|                            | `resetCursor` = _false_         | `resetCursor` = _true_ (default) |
| -------------------------- | ------------------------------- | -------------------------------- |
| **Currently playing**      | Has no effect.                  | Plays from the start.            |
| **Paused**                 | Resumes from last frame played. | Plays from the start.            |
| **Finished (Non-looping)** | Plays from the start.           | Plays from the start.            |

## Looping animations

By default, animations are played in a loop that keeps repeating the animation forever.

Change this setting by setting the `loop` property in the `state` object.

```ts
Animator.create(shark, {
	states: [
		{
			clip: 'bite',
			playing: true,
			loop: false,
		},
	],
})
```

If `loop` is set to _false_, the animation plays just once and then stops, staying on the posture of the last frame.

## Stop an animation

To stop all animations that an entity is playing, use `Animator.stopAllAnimations()`.

```ts
Animator.stopAllAnimations(shark)
```

`Animator.stopAllAnimations` requires the following parameters:

* `entity`: The entity of the `Animator` component that you want to affect.
* `resetCursor`: _(optional)_ If _true_, it returns to the posture in the first frame of the animation. If _false_, stays paused in its current posture. Default: _true_.


> [!WARNING]
> **📔 Note**: When playing an animation with `Animator.playSingleAnimation`, this function handles stopping all other animations behind the scenes. You don't need to explicitly stop other animations in that case.


When an animation finishes playing a non-looping animation, by default the 3D model remains in the last posture it had. The `shouldReset` property controls what happens when a stopped animation is played again: if _true_, the animation is restored to its initial state (its first frame, or its last frame if playing with a negative `speed`) whenever it changes from stopped to playing. If _false_ (the default), it resumes from where it was.

```ts
Animator.create(shark, {
	states: [
		{
			clip: 'bite',
			playing: true,
			shouldReset: true,
			loop: true,
		},
	],
})
```

You can also use `Animator.stopAllAnimations()` at any time to explicitly set the posture back to the first frame in the animation.


> [!WARNING]
> **📔 Note**: Resetting the posture is an abrupt change. If you want to make the model transition smoothly into another posture, play the other animation and blend between the two by gradually shifting their `weight` properties. See [Animation weight](#animation-weight).


## Detect when an animation finishes

When a non-looping animation finishes playing, the engine sets that animation state's `playing` property back to _false_. Your scene's code can read this value to know when the animation ended, for example to chain another animation right after it.

```ts
let wasPlaying = false

engine.addSystem(() => {
	const animator = Animator.get(shark)
	const biteState = animator.states.find((state) => state.clip === 'bite')
	const isPlaying = biteState?.playing ?? false

	if (wasPlaying && !isPlaying) {
		console.log('bite animation finished')
		// chain the next animation
		Animator.playSingleAnimation(shark, 'swim')
	}

	wasPlaying = isPlaying
})
```


> [!WARNING]
> **📔 Note**: When polling the animation state every frame, always read it through `Animator.get()` (read-only). Don't use `Animator.getClip()` or `Animator.getMutable()` for polling: these return a mutable version of the component, which marks it as changed on every frame and causes unnecessary synchronization work.
> 
> The `playing` property is only flipped by the engine when the animation ends by itself. Looping animations play until stopped, so they never flip the property on their own, and animations with `speed` set to 0 never finish.



> [!WARNING]
> **📔 Note**: This feature is only supported in the Desktop client.


## Handle multiple animations

If a 3D model has multiple animations packed into it, a single `Animator` component can deal with all of them.

```ts
// Create entity
const shark = engine.addEntity()

// Add a 3D model to it
GltfContainer.create(shark, {
	src: 'models/shark.glb'
})

Animator.create(shark, {
	states:[{
			clip: "swim",
			playing: true,
			loop: true
		}, {
			clip: "bite",
			playing: true,
			loop: true
		}
	]
})
```

In the example above, two animations are handled by separate `state` objects, and they are then both assigned to the same `Animator` component.

Each bone in an animation can only be affected by one animation at a time, unless these animations have a `weight` that adds up to a value of 1 or less.

If one animation only affects a character's legs, and another only affects a character's head, then they can be played at the same time without any issue. But if they both affect the character's legs, then you must either only play one at a time, or play them with lower `weight` values.

If in the above example, the `bite` animation only affects the shark's mouth, and the `swim` animation only affects the bones of the shark's spine, then they can both be played at the same time.


> [!WARNING]
> **📔 Note**: `Animator.playSingleAnimation()` stops all other animations that the entity is currently playing. To play multiple animations at the same time, modify the `playing` property in the animation states manually.


## Animation speed

Change the speed at which an animation is played by changing the `speed` property. The value of the speed is 1 by default.

```ts
Animator.create(shark, {
	states: [
		{
			clip: 'swim',
			playing: true,
			loop: true,
			speed: 2,
		},
	],
})
```

Set the speed lower than 1 to play it slower, for example to 0.5 to play it at half the speed. Set it higher than 1 to play it faster, for example to 2 to play it at double the speed.

```ts
const swimAnim = Animator.getClip(sharkEntity, 'swim')

swimAnim.speed = 0.5
```

## Animation weight

The `weight` property allows a single model to carry out multiple animations at once, calculating a weighted average of all the movements involved in the animation. The value of `weight` determines how much importance that animation will be given in the average.

By default, `weight` is equal to _1_. The value of `weight` can't be any higher than _1_.

```ts
Animator.create(shark, {
	states: [
		{
			clip: 'swim',
			playing: true,
			loop: true,
			weight: 0.2,
		},
	],
})
```

The `weight` value of all active animations in an entity should add up to 1 at all times. If it adds up to less than 1, the weighted average will be using the default position of the armature for the remaining part of the calculation.

For example, in the code example above, we're playing the _swim_ animation, that only has a `weight` of _0.2_. This swimming movement will be quite subtle: only 20% of the intensity that the animation defines. The remaining 80% of the calculation takes values from the default posture of the armature.

The `weight` property can be used in interesting ways, for example the `weight` property of _swim_ could be set in proportion to how fast the shark is swimming, so you don't need to create multiple animations for fast and slow swimming.

You could also change the `weight` value gradually when starting and stopping an animation to give it a more natural transition and to avoid jumps from the default pose to the first pose in the animation.


> [!WARNING]
> **📔 Note**: The added `weight` value of all animations that are acting on a 3D model's bone can't be more than 1. If more than one animation is affecting the same bones at the same time, they need to have their weight set to values that add to less than 1.


```ts
const swimAnim = Animator.getClip(sharkEntity, 'swim')

swimAnim.weight = 0.5
```
