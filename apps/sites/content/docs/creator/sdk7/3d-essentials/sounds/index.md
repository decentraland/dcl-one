> Learn how to add sounds to your scene.

# Sounds

Sound is a great way to provide feedback to player actions and events, background sounds can also give your scene more context and improve the player's immersion into it.


> [!WARNING]
> **📔 Note**: Keep in mind that sounds are only heard by players who are standing within the parcels that make up the scene where the sound was generated, even if they would otherwise be in hearing range. Players can also chose to turn off all sounds on their settings.


Supported sound formats vary depending on the browser, but it's recommended to use _.mp3_.

_.wav_ files are also supported but not generally recommended as they are significantly heavier.

## Play sounds

The easiest way to play a sound is to add an **Audio Source** component visually on the [Scene Editor in Creator Hub](/docs/creator/scene-editor/get-started/about-editor/) and set it to **Start Playing** and **Loop**. See [Add Components](/docs/creator/scene-editor/build/components/#add-components).

![](/docs/creator/images/editor/AudioSource-component.webp)

You can also trigger the playing of a sound in a no-code way via **Actions**, see [Make any item smart](/docs/creator/scene-editor/interactivity/make-any-item-smart/).

To play a sound via code, use the `AudioSource.playSound` function.

```ts
// Create entity
const sourceEntity = engine.addEntity()

// Play sound
AudioSource.playSound(sourceEntity, 'assets/sounds/sound-effect.mp3')
```

The sound file must be inside the project folder. In the example above, the audio file is located in an `assets/sounds` folder, which is located at root level of the scene project folder.


> [!WARNING]
> **📔 Note**: The `AudioSource` component must be imported via
> 
> > `import { AudioSource } from "@dcl/sdk/ecs"`
> 
> See [Imports](/docs/creator/sdk7/getting-started/coding-scenes/#imports) for how to handle these easily.


The `AudioSource.playSound()` function takes the following arguments:

* `entity`: On what entity to apply the sound. The sound will be heard from this entity's position, meaning it gets louder as the player approaches it.
* `src`: The location of the sound file within your project.


> [!NOTE]
> **💡 Tip**: For more clarity, we recommend keeping your sound files separate in a `assets/sounds` folder inside your scene.


* `resetCursor`: _(optional)_ If true, the sound always starts from the beginning. Otherwise it continues from the current cursor position. Useful for pausing and resuming.

Another way to play sounds is to manually create an `AudioSource` component on an entity. Use this approach to have more control over the sound, for example to make it loop or set the volume.

```ts
// Create entity
const sourceEntity = engine.addEntity()

// Create AudioSource component
AudioSource.create(sourceEntity, {
	audioClipUrl: 'sounds/sound-effect.mp3',
	loop: true,
	playing: true,
})
```

The following properties can be set:

* `audioClipUrl`: The location of the sound file within your project.
* `playing`: If true, the sound starts playing. You can create a sound with `playing` set to false, and then set it to true at a later time.
* `volume`: _(optional)_ The volume of the sound file. 1 by default, which is full volume.
* `pitch`: _(optional)_ Modify the pitch of a sound. 1 is the default, make it lower for a deeper sound and higher for a higher pitch sound.


> [!NOTE]
> **💡 Tip**: To prevent a sound effect from becoming too repetitive during a game, it's useful to randomize some slight variations to the sound's pitch every time it plays.


* `currentTime`: _(optional)_ 0 by default. Set this value to avoid starting from the beginning of the sound file.

Each entity can only have a single `AudioSource` component, that can only play a single clip at a time. This limitation can be easily overcome by modifying the audio source at the time of playing a new sound, or by including multiple invisible child entities, each with their own sound.


> [!WARNING]
> **📔 Note**: Sounds are played on each player's local instance. Other nearby players won't hear the same sounds unless their local scene explicitly plays them too.


### Pre Loading a Sound

If an entity uses a sound, but is not played immediately at scene runtime, it might take some time to download. It can be available at scene runtime by using the `AssetLoad` component. 

```ts
import { AssetLoad } from "@dcl/sdk/ecs"

AssetLoad.create(engine.RootEntity, {
  assets: [
    "assets/scene/bundle1/explosionSound.mp3",
  ],
})
```
For more information, check the [Pre Load Resources](/docs/creator/sdk7/optimizing/pre-load-resources/) documentation.

## Stopping sounds

To stop an entity from playing its sound, use the `AudioSource.stopSound()` function. You only need to specify the entity, since each entity has a single `AudioSource` component, and each `AudioSource` component plays a single file at a time.

```ts
AudioSource.stopSound(sourceEntity)
```

Another way to stop a sound is to set the `playing` property to false.

```ts
// Create entity
const sourceEntity = engine.addEntity()

// Create AudioSource component
AudioSource.create(sourceEntity, {
	audioClipUrl: 'sounds/explosion.mp3',
	playing: true,
})

// Define a simple function
function stopSound(entity: Entity) {
	// fetch mutable version of audio source component
	const audioSource = AudioSource.getMutable(entity)

	// modify its playing value
	audioSource.playing = false
}

// call function
stopSound(sourceEntity)
```

## Detect when a sound finishes

When a non-looping sound finishes playing on its own, the engine sets the `playing` property of the `AudioSource` component back to _false_. Your scene's code can read this value to know when the sound ended, for example to chain another sound or action right after it.

```ts
let wasPlaying = false

engine.addSystem(() => {
	const audio = AudioSource.get(sourceEntity)
	const isPlaying = audio.playing ?? false

	if (wasPlaying && !isPlaying) {
		console.log('sound finished playing')
		// react here: play another sound, advance a sequence, etc.
	}

	wasPlaying = isPlaying
})
```


> [!WARNING]
> **📔 Note**: When polling the component every frame, always read it with `AudioSource.get()` (read-only). Using `AudioSource.getMutable()` would mark the component as changed on every frame, causing unnecessary synchronization work.
> 
> The `playing` property is only flipped by the engine when the sound ends by itself. If the scene stops the sound explicitly (via `AudioSource.stopSound()` or by setting `playing` to _false_), that change comes from your own code, and looping sounds play until stopped, so they never flip the property on their own.


Alternatively, use `audioEventsSystem.registerAudioEventsEntity` to define a function that runs every time the playback state of the entity's audio changes, just like [video events](/docs/creator/sdk7/media/video-playing/#video-events) for videos. The function receives an event with a `state` field, using the `MediaState` enum.

```ts
import { engine, AudioSource, audioEventsSystem, MediaState } from '@dcl/sdk/ecs'

// ... Create sourceEntity with an AudioSource component ...

audioEventsSystem.registerAudioEventsEntity(sourceEntity, (audioEvent) => {
	switch (audioEvent.state) {
		case MediaState.MS_LOADING:
			console.log('audio event - sound is LOADING')
			break
		case MediaState.MS_READY:
			console.log('audio event - sound is READY (loaded, or finished playing)')
			break
		case MediaState.MS_PLAYING:
			console.log('audio event - sound started PLAYING')
			break
		case MediaState.MS_ERROR:
			console.log('audio event - sound ERROR (e.g. the file failed to load)')
			break
	}
})
```

A transition from `MS_PLAYING` to `MS_READY` means the sound stopped playing. The full `MediaState` enum also includes `MS_NONE`, `MS_LOADING`, `MS_PAUSED`, `MS_BUFFERING`, `MS_SEEKING` and `MS_ERROR`, and each event carries a `timestamp` of when the state change occurred. You can also query the latest reported state at any time with `audioEventsSystem.getAudioState(entity)`, unregister the callback with `audioEventsSystem.removeAudioEventsEntity(entity)`, or check whether an entity is registered with `audioEventsSystem.hasAudioEventsEntity(entity)`. The same system also works for entities with an [`AudioStream`](/docs/creator/sdk7/media/audio-streaming/) component.


> [!WARNING]
> **📔 Note**: These features are only supported in the Desktop client.


## Looping

To keep a sound playing in a continuous loop, set the `loop` field of the `AudioSource` component to _true_ before you start playing it.

```ts
// Create entity
const sourceEntity = engine.addEntity()

// Create AudioSource component
AudioSource.create(sourceEntity, {
	audioClipUrl: 'sounds/sound-effect.mp3',
	playing: true,
	loop: true,
})
```

Looping sounds is especially useful for adding background music or other background sounds.

## Set volume

You can set the `volume` property of the `AudioSource` component to change the volume of a sound.

The volume is expressed as a number from _0_ to _1_.

```ts
// Create entity
const sourceEntity = engine.addEntity()

// Create AudioSource component
AudioSource.create(sourceEntity, {
	audioClipUrl: 'sounds/sound-effect.mp3',
	playing: true,
	volume: 0.5,
})
```


> [!WARNING]
> **📔 Note**: Of course, the volume of a sound is also affected by the distance of the player from the audio source. As the player walks away, the volume will be lower.


## Global sounds

By default, all sounds from an `AudioSource` are positional. This means they appear to generate from the position of the `Transform` component, and will sound louder as the player walks closer. But you can also configure a sound to be global, so that the volume is constant, no matter where the player is standing. This is ideal for using on background music, notification sounds, and other non-positional sound.


> [!WARNING]
> **📔 Note**: Global Sounds are a feature that's only supported in the DCL 2.0 desktop client.


To make a sound global, set the `global` property to _true_.

```ts
AudioSource.create(sourceEntity, {
	audioClipUrl: 'sounds/music.mp3',
	playing: true,
	global: true,
})
```

## Play a segment of a sound

To play a segment of a longer sound file, use the `playSoundSegment()` in the SDK Utils library. See [SDK7 Utils](https://github.com/decentraland/sdk7-utils).

You can also achieve this by explicitly set the `currentTime` property on an `AudioSource` component, and then stopping it after waiting for a period of time.

## Audio streaming

See [Audio streaming](/docs/creator/sdk7/media/audio-streaming/) to learn how you can play a live audio stream from an external source.

## Audio analysis

See [Audio analysis](/docs/creator/sdk7/media/audio-analysis/) to learn how to read real-time amplitude and frequency data from an `AudioSource` and drive reactive visuals from it, like cubes that pulse with the bass or equalizer-style bar visualizers.
