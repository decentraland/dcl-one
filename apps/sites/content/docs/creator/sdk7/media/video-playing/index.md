> Stream video into a scene

# Video Playing

There are three different ways you can show a video in a scene:

- Upload a video file as part of the scene contents
- Stream the video from an external source
- Stream live via Decentraland cast


> [!NOTE]
> **💡 Tip**: In the [Scene Editor in Creator Hub](/docs/creator/scene-editor/get-started/about-editor/), you can use an **Video Player** [Smart Item](/docs/creator/scene-editor/interactivity/smart-items/) for a no-code way to achieve this.


In all cases, you'll need:

- An entity with a [primitive shape](/docs/creator/sdk7/3d-essentials/shape-components/) like a plane, cube, or even a cone.
- A [material](/docs/creator/sdk7/3d-essentials/materials/) with a `VideoTexture` assigned to its texture
- A `VideoPlayer` component to control the state of the video.

## Performance considerations

Keep in mind that streaming video demands a significant effort from the player's machine. It's recommended to avoid playing more than one video at a time.

If too many videos are playing at the same time in your scene, some will be paused by the engine. The priority for pausing a screen is determined based on several factors, including proximity to the player, size, and whether the screen is in the player's field of view. The maximum number of simultaneous videos depends on the player's quality settings.

- Low: 1
- Medium: 5
- High: 10

We also recommend starting to play the video when the player is near or performs an action to do that. Starting to play a video when your scene is loaded far in the horizon will unnecessarily affect performance while players visit neighboring scenes.

Also avoid streaming videos in very high resolution. Don't use anything above _HD_.

It's also ideal to play videos on Basic (unlit) materials, to reduce the performance load, as is the case on all of the example snippets below.

## Show a video

The following instructions apply to all three video showing options:

1. Create an entity to serve as the video screen. Give this entity a `MeshRenderer` component so that it has a visible shape.
2. Create a `VideoPlayer` component, either referencing a streaming URL or a path to a video file. Here you can also set the video's `playing` state and its volume. This component can be assigned to the video screen entity or to any other entity in the scene.
3. Create a `VideoTexture` object, and in its `videoPlayerEntity` property assign the entity that owns the `VideoPlayer` component.
4. Create a `Material`, assign it to the screen entity, and set its `texture` to the `VideoTexture` you created.

This example uses a video that's locally stored in a `/videos` folder in the scene project:

```ts
// #1
const screen = engine.addEntity()
MeshRenderer.setPlane(screen)
Transform.create(screen, { position: { x: 4, y: 1, z: 4 } })

// #2
VideoPlayer.create(screen, {
	src: 'videos/myVideo.mp4',
	playing: true,
})

// #3
const videoTexture = Material.Texture.Video({ videoPlayerEntity: screen })

// #4
Material.setBasicMaterial(screen, {
	texture: videoTexture,
})
```

To use a video from an external streaming URL, change step 2 so that the `src` property in the `VideoPlayer` component references the streaming URL.

```ts
// #2
VideoPlayer.create(screen, {
	src: 'https://vz-7c61c1b5-d59.b-cdn.net/ccea595a-b910-4de6-b160-092819db021d/playlist.m3u8',
	playing: true,
})
```

See [Streaming using Decentraland cast](/docs/creator/sdk7/media/video-playing/#streaming-using-decentraland-cast) for details on how to use this third alternative method.

## About External Streaming

The source of the streaming must be an _https_ URL (_http_ URLs aren't supported).

You should be able to paste a URL pointing to a video from most popular video streaming sites. Be mindful of the terms of service with these platforms.

To stream from a video file you have on your local machine, the easiest path is to upload this video to a public Google Drive and paste the link.

Another option is to use a managed hosting provider like [Vimeo](https://vimeo.com/) , [Livepeer Studio](https://livepeer.studio/) or [Serraform](https://serraform.gitbook.io/streaming-docs/guides/decentraland-playback) where you pay a fee to the provider to manage all the streaming infrastructure.

The most powerful approach is to set up your own server, using free software but paying for hosting on a platform like [Digital Ocean](https://try.digitalocean.com/developerbrand/?_campaign=emea_brand_kw_en_cpc&_adgroup=digitalocean_exact_exact&_keyword=digitalocean&_device=c&_adposition=&_content=conversion&_medium=cpc&_source=bing&msclkid=160bfc160a2a1bab9bbf9933594bd9c5&utm_source=bing&utm_medium=cpc&utm_campaign=emea_brand_kw_en_cpc&utm_term=digitalocean&utm_content=DigitalOcean%20Exact_Exact) or [Cloudflare](https://www.cloudflare.com/products/cloudflare-stream/). You can deploy something like a [Node Media Server](https://github.com/illuspas/Node-Media-Server), which provides most of what you need out of the box.

All these options have pros and cons for different scenarios. You should evaluate what's best for you taking into account your needs, technical skills and budget.

## Setting up OBS for successful streaming

[OBS](https://obsproject.com/) is a popular and free tool for managing your streams.

Whether you are using a venue’s stream key or your own RTMP server, your settings in OBS are important for the success of your stream. You should aim for a solid, consistent connection.

### Simple OBS set-up

The following simple set-up is recommended:

- Bitrate 2500kbps (which will work with all Decentraland venues)
- Audio bitrate 160kbps
- Video encoder preset: Hardware NVENC
- Audio Encoder AAC
- Broadest Resolution: 720 (any greater causes issues in DCL)
- Frame rate 30fps


> [!WARNING]
> **📔 Note**: Make sure you disable Multitrack streaming in OBS. You can find this in Settings > Stream.


### Advice for new streamers

- Early sound checks are essential to test your set up with the venue.
- Small errors like a digit wrong in the stream key are the most likely to mess up the stream.
- Do not go above 720 resolution or a bitrate of 2500 kbps.

## Live streaming

You can livestream from your camera or share your screen using the [Live streaming](/docs/creator/scene-editor/live-ops/live-streaming/) feature of the [Admin tools](/docs/creator/scene-editor/live-ops/scene-admin/) smart item.

This streaming method uses the same comms architecture used for live communications between players. It's easy to set up and has much less delay than streaming from external sources.

1. Add an [Admin tools](/docs/creator/scene-editor/live-ops/scene-admin/) smart item to your scene, as well as a [Video player](/docs/creator/scene-editor/interactivity/video-screen/) smart item.
2. Publish your scene, either to a World or to Genesis City.
3. Enter the scene as a player with the permission to use the Admin tools.
4. Open the Admin console, select the **Video** tab, then select the **Live** functionality and click the **Get Stream Key** button.
5. Copy the **Server URL** and _Streaming key_\* to your streaming software (for example OBS).
6. Press the **Activate** button to start streaming.

Instead of adding a Video player smart item to your scene, you can also use the URL `livekit-video://current-stream` as the video source, to play the stream in your scene. You will still need the Admin tools to get the stream key.

```ts
// #1
const screen = engine.addEntity()
MeshRenderer.setPlane(screen)
Transform.create(screen, { position: { x: 4, y: 1, z: 4 } })

// #2
VideoPlayer.create(screen, {
	src: `livekit-video://current-stream`,
	playing: true,
})

// #3
const videoTexture = Material.Texture.Video({ videoPlayerEntity: screen })

// #4
Material.setBasicMaterial(screen, {
	texture: videoTexture,
})
```

## Video Materials

Most of the time, you'll want to play videos on an unlit [Basic material](/docs/creator/sdk7/3d-essentials/materials/#unlit-materials), rather than a PBR material. This results in a much brighter and crisper image and is better for performance.

```ts
Material.setBasicMaterial(screen, {
	texture: videoTexture,
})
```

It's usually recommended to play videos on Basic unlit materials, as this is better for performance. However, if you want to project a video onto a PBR material, keep in mind that the default properties make the video look rather opaque. You can enhance this by altering other properties of the material. Here are some recommended settings to make the video stand out more:

```ts
Material.setPbrMaterial(screen, {
	texture: videoTexture,
	roughness: 1.0,
	specularIntensity: 0,
	metallic: 0,
	emissiveTexture: videoTexture,
	emissiveIntensity: 0.6,
	emissiveColor: Color3.White(),
})
```


> [!NOTE]
> **💡 Tip**: Since the video is a texture that's added to a material, you can also experiment with other properties of materials, like tinting it with a color, or adding other texture layers, for example to produce a dirty screen effect.
> 
> See [materials](/docs/creator/sdk7/3d-essentials/materials/) for more details.


## About Video Files

The following file formats are supported:

- _.mp4_
- _.ogg_
- _.webm_

Keep in mind that a video file adds to the total size of the scene, which makes the scene take longer to download for players walking into your scene. The video size might also make you go over the [scene limitations](/docs/creator/sdk7/optimizing/scene-limitations/), as you have a maximum of 15 MB per parcel to use. We recommend compressing the video as much as possible, so that it's less of a problem.

We also recommend starting to play the video when the player is nearby or performs an action to trigger it. Starting to play a video when your scene is loaded far in the horizon will unnecessarily affect performance while players visit neighboring scenes.

## Start pause and stop a video

To start playing the video or pause it, set the `playing` property to _true_ or _false_. If `playing` is set to false, the video is paused at the last frame shown.

You can make a screen toggleable by adding a pointer event to it as shown below:

```ts
pointerEventsSystem.onPointerDown(
	{
		entity: screen,
		opts: { button: InputAction.IA_POINTER, hoverText: 'Play/Pause' },
	},
	function () {
		const videoPlayer = VideoPlayer.getMutable(screen)
		videoPlayer.playing = !videoPlayer.playing
	}
)
```

To stop the video and send it back to the first frame, set the `position` property to 0. In the following example, clicking on the video stops it.

```ts
pointerEventsSystem.onPointerDown(
	{
		entity: screen,
		opts: { button: InputAction.IA_POINTER, hoverText: 'STOP' },
	},
	function () {
		const videoPlayer = VideoPlayer.getMutable(screen)
		videoPlayer.playing = false
		videoPlayer.position = 0
	}
)
```

## Configure the video player

The following optional properties are available to set on the `VideoPlayer` component:

- `playing`: Determines if the video is currently playing. If false, the video is paused.


> [!WARNING]
> **📔 Note**: Only a limited number of videos can play at the same time, depending on the player's quality settings. If the scene plays more than that, the engine pauses the lowest-priority ones. See [Performance considerations](/docs/creator/sdk7/media/video-playing/#performance-considerations).


- `playbackRate`: Changes the speed at which the video is played. _1_ by default.
- `volume`: Lets you change the volume of the audio. _1_ by default.
- `position`: Allows you to set a different starting position on the video. It's expressed in seconds after the video's original beginning. _0_ by default, which makes it start at the actual start of the video.
- `loop`: Boolean that determines if the video is played continuously in a loop, or if it stops after playing once. _false_ by default.

## Play multiple videos

To avoid running into performance problems, keep the amount of different videos playing at the same time low; the engine only allows a limited number of simultaneous videos, depending on the player's quality settings (see [Performance considerations](/docs/creator/sdk7/media/video-playing/#performance-considerations)). A scene can however play multiple copies of the same video texture on several different screens. This is not restricted, as it impacts performance considerably less than playing separate videos. To play the same video on multiple entities, simply assign the same instance of the video texture object to the `Material` components of each screen entity.

```ts
// #1
const screen1 = engine.addEntity()
MeshRenderer.setPlane(screen1)
Transform.create(screen1, { position: { x: 4, y: 1, z: 4 } })

const screen2 = engine.addEntity()
MeshRenderer.setPlane(screen2)
Transform.create(screen2, { position: { x: 6, y: 1, z: 4 } })

// #2
VideoPlayer.create(screen1, {
	src: 'https://vz-7c61c1b5-d59.b-cdn.net/ccea595a-b910-4de6-b160-092819db021d/playlist.m3u8',
	playing: true,
})

// #3
const videoTexture = Material.Texture.Video({ videoPlayerEntity: screen1 })

// #4
Material.setBasicMaterial(screen1, {
	texture: videoTexture,
})

Material.setBasicMaterial(screen2, {
	texture: videoTexture,
})
```

Note that in the example above, it's only necessary to create one `VideoPlayer` component, which controls the state of both video screens. In this case, the component is assigned to the `screen1` entity, but it could also be assigned to any other entity in the scene, not necessarily one of the screens.

## Video events

Easily handle state changes in a video, to respond to when a video starts playing, is paused, etc. This can be used for example to play animations in perfect sync with a video, ensuring they start at the same time as the video.

Use `videoEventsSystem.registerVideoEventsEntity` to define a function that runs every time the state of the video assigned to an entity changes. Every time the state changes, your function can check the new state and respond accordingly.

```ts
import {
	engine,
	Entity,
	VideoPlayer,
	videoEventsSystem,
	VideoState,
} from '@dcl/sdk/ecs'

// ... Create videoPlayerEntity with VideoPlayer component, Transform, MeshRenderer.setPlane(), etc. ...

videoEventsSystem.registerVideoEventsEntity(
	videoPlayerEntity,
	function (videoEvent) {
		console.log(
			'video event - state: ' +
				videoEvent.state +
				'\ncurrent offset:' +
				videoEvent.currentOffset +
				'\nvideo length:' +
				videoEvent.videoLength
		)

		switch (videoEvent.state) {
			case VideoState.VS_READY:
				console.log('video event - video is READY')
				break
			case VideoState.VS_NONE:
				console.log('video event - video is in NO STATE')
				break
			case VideoState.VS_ERROR:
				console.log('video event - video ERROR')
				break
			case VideoState.VS_SEEKING:
				console.log('video event - video is SEEKING')
				break
			case VideoState.VS_LOADING:
				console.log('video event - video is LOADING')
				break
			case VideoState.VS_BUFFERING:
				console.log('video event - video is BUFFERING')
				break
			case VideoState.VS_PLAYING:
				console.log('video event - video started PLAYING')
				break
			case VideoState.VS_PAUSED:
				console.log('video event - video is PAUSED')
				break
		}
	}
)
```

The videoEvent object passed as an input for the function contains the following properties:

- `currentOffset` (_number_): The current value of the `position` property on the video. This value shows seconds after the video's original beginning. _-1_ if the video hasn't started playing.
- `state`: The new video status, expressed as a value from the `VideoState` enum. This enum can hold the following possible values:
  - `VideoState.VS_READY`
  - `VideoState.VS_NONE`
  - `VideoState.VS_ERROR`
  - `VideoState.VS_SEEKING`
  - `VideoState.VS_LOADING`
  - `VideoState.VS_BUFFERING`
  - `VideoState.VS_PLAYING`
  - `VideoState.VS_PAUSED`
- `videoLength` (_number_ ): The length in seconds of the entire video. _-1_ if length is unknown.
- `timestamp` ( _number_): A _lamport_ timestamp that is incremented every time that the video changes state.
- `tickNumber` (_number_): The time at which the event occurred, expressed as counting ticks since the scene started running.

If you re-register a callback on an entity that already has one (for example, to change what happens on state change), the new callback replaces the old one, and the last-reported state is preserved. This means the new callback is not called again for a state that was already reported to the previous callback.

### Check if an entity is registered

Use `videoEventsSystem.hasVideoEventsEntity()` to check whether an entity already has a registered video events callback.

```ts
if (videoEventsSystem.hasVideoEventsEntity(videoPlayerEntity)) {
	console.log('This entity already has a video event listener')
}
```

### Latest video event

Query a video for its last state change by using `videoEventsSystem.getVideoState()`. This returns the latest `VideoEvent` value for the entity, or `undefined` if no event has been reported yet. You can call this from a system to poll the video state each frame without registering a callback.

```ts
function mySystem() {
	const latestVideoEvent = videoEventsSystem.getVideoState(videoPlayerEntity)
	if (!latestVideoEvent) return

	console.log(`state: ${latestVideoEvent.state}
    \ncurrentOffset: ${latestVideoEvent.currentOffset}
    \nvideoLength: ${latestVideoEvent.videoLength}`)
}
```

## Alpha masks on videos

A neat trick to have non-rectangular video screens is to apply an alpha texture on top of a plane. You can cut away part of the plane into whatever shape you want.

Use the following image to cut your video into a circular shape, with transparent corners.

![](/docs/creator/images/circle_mask.webp)

```ts
const videoTexture = Material.Texture.Video({
	videoPlayerEntity: screen,
})
const alphaMask = Material.Texture.Common({
	src: 'assets/scene/circle_mask.png',
	wrapMode: TextureWrapMode.TWM_MIRROR,
})

Material.setBasicMaterial(screen, {
	texture: videoTexture,
	alphaTexture: alphaMask,
})
```

![](/docs/creator/images/circular-video-screen.webp)


> [!WARNING]
> **📔 Note**: In previous versions, the `alphaTexture` property was only present in PBR materials. Currently, it only works in basic materials.


## Play a video on a glTF model

You can play a video on a _glTF_ model by using the [GltfNodeModifiers](/docs/creator/sdk7/3d-essentials/materials/#modify-gltf-materials) component. See [Modify glTF materials](/docs/creator/sdk7/3d-essentials/materials/#modify-gltf-materials) for more details.

This allows you to play your videos on any shape, not just planes. For example, you can play videos on a curved screen, or even the entire body of an NPC.

```ts
const myEntity = engine.addEntity()

GltfContainer.create(myEntity, {
	src: 'models/myModel.glb',
})

Transform.create(myEntity, {
	position: Vector3.create(4, 0, 4),
})

VideoPlayer.create(myEntity, {
	src: 'https://vz-7c61c1b5-d59.b-cdn.net/ccea595a-b910-4de6-b160-092819db021d/playlist.m3u8',
	playing: true,
})

GltfNodeModifiers.create(myEntity, {
	modifiers: [
		{
			path: '',
			material: {
				material: {
					$case: 'pbr',
					pbr: {
						texture: Material.Texture.Video({
							videoPlayerEntity: myEntity,
						}),
					},
				},
			},
		},
	],
})
```

The mapping of the video will follow the original UV mapping that the model uses. This means that if the model has a texture that is mapped to a specific part of the model, the video will be mapped to that same part.

You can also use the `GltfNodeModifiers` component to play a video only on a specific mesh inside the model. For example, you can play it on a specific wall of a building, even though the model spans the entire building. See [Modify glTF materials](/docs/creator/sdk7/3d-essentials/materials/#modify-gltf-materials) for more details.

## Spatial audio

By default, the video from a `VideoPlayer` component is global, meaning it will be heard at a consistent volume throughout your entire scene. If a player steps out of the scene, they will not hear the streaming at all.

To make the audio spatial, set the `spatial` property to _true_.

```ts
VideoPlayer.create(entity, {
	src: 'https://player.vimeo.com/progressive_redirect/playback/1145666916/rendition/540p/file.mp4%20%28540p%29.mp4?loc=external&signature=db1cd6946851313cb8f7be60d1f6c30af0902bcc46fdae0ba2a06e5fdf44c329',
	playing: true,
	spatial: true,
})
```

The video will now be heard from the position of the entity that owns the `VideoPlayer` component, and will be louder as the player approaches it.

Control the spatial audio with the following properties:

- `spatialMinDistance`: The minimum distance at which audio becomes spatial. If the player is closer, the audio will be heard at full volume. _0_ by default.
- `spatialMaxDistance`: The maximum distance at which the audio is heard. If the player is further away, the audio will be heard at 0 volume. _60_ by default

```ts
const videoPlayerEntity = engine.addEntity()

Transform.create(videoPlayerEntity, {
	position: Vector3.create(8, 2, 8),
})

VideoPlayer.create(videoPlayerEntity, {
	src: 'https://player.vimeo.com/progressive_redirect/playback/1145666916/rendition/540p/file.mp4%20%28540p%29.mp4?loc=external&signature=db1cd6946851313cb8f7be60d1f6c30af0902bcc46fdae0ba2a06e5fdf44c329',
	playing: true,
	spatial: true,
	spatialMinDistance: 5,
	spatialMaxDistance: 10,
})

MeshRenderer.setPlane(videoPlayerEntity)

Material.setBasicMaterial(videoPlayerEntity, {
	texture: Material.Texture.Video({ videoPlayerEntity: videoPlayerEntity }),
})
```


> [!WARNING]
> **📔 Note**: Some video formats don't support spatial audio. Make sure the stream is encoded in _mp4_, _m4a_, or _mov_.


## Audio analysis

You can read real-time amplitude and frequency data from the audio track of a `VideoPlayer` entity to drive reactive visuals that sync with the video's soundtrack. See [Audio analysis](/docs/creator/sdk7/media/audio-analysis/).


> [!NOTE]
> **💡 Tip**: To see video played on a GLTF model rather than a primitive, see the [`74,-8-gltfnodemodifier`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/74,-8-gltfnodemodifier) test scene, which drives an HLS stream onto specific GLTF nodes using `GltfNodeModifiers` video textures.

