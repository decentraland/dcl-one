> 将视频流传输到场景中

# 视频播放

在场景中显示视频有三种不同方式：

* 将视频文件作为场景内容的一部分上传
* 从外部来源串流视频
* 通过 Decentraland Cast 进行直播


> [!NOTE]
> **💡 提示**：在 [Creator Hub 中的场景编辑器](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/ru-men/about-editor/)，你可以使用一个 **视频播放器** [智能物品](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/smart-items/) 以无代码方式实现这一点。


在所有情况下，你都需要：

* 一个带有 [基础形状](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/shape-components/) 的实体，例如平面、立方体，甚至圆锥体。
* A [材质](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/materials/) 并带有一个 `VideoTexture` 被分配到其纹理的
* A `VideoPlayer` 组件，用于控制视频状态。

## 性能注意事项

请记住，串流视频会对玩家机器造成较大的负载。建议一次不要播放超过一个视频。

如果你的场景中同一时间播放了太多视频，其中一些会被引擎暂停。屏幕暂停的优先级会根据多个因素来决定，包括与玩家的距离、大小，以及屏幕是否位于玩家的视野范围内。可同时播放的视频最大数量取决于玩家的画质设置。

* 低：1
* 中：5
* 高：10

我们还建议在玩家靠近时或执行相应操作时再开始播放视频。如果在场景远在地平线处加载时就开始播放视频，会在玩家访问相邻场景时不必要地影响性能。

也要避免以非常高的分辨率串流视频。不要使用高于 *HD*.

理想情况下，也应在 Basic（无光照）材质上播放视频，以降低性能负载，下面所有示例片段都是如此。

## 显示视频

以下说明适用于这三种视频显示方式：

1. 创建一个实体作为视频屏幕。给这个实体一个 `MeshRenderer` 组件，使其具有可见形状。
2. 创建一个 `VideoPlayer` 组件，引用一个串流 URL 或视频文件路径。在这里你还可以设置视频的 `playing` 状态及其音量。这个组件可以分配给视频屏幕实体，也可以分配给场景中的任何其他实体。
3. 创建一个 `VideoTexture` 对象，并在其 `videoPlayerEntity` 属性中指定拥有该 `VideoPlayer` 组件的实体时，组件不会被激活。
4. 创建一个 `材质`，将其分配给屏幕实体，并设置其 `纹理` 到 `VideoTexture` 为你创建的内容。

此示例使用的是本地存储在 `/videos` 文件夹中的视频：

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

要使用来自外部串流 URL 的视频，请将第 2 步改为 `src` 组件中的 `VideoPlayer` 组件引用该串流 URL。

```ts
// #2
VideoPlayer.create(screen, {
	src: 'https://vz-7c61c1b5-d59.b-cdn.net/ccea595a-b910-4de6-b160-092819db021d/playlist.m3u8',
	playing: true,
})
```

请参阅 [直播流](#live-streaming) ，了解如何使用这种第三种替代方法的详细信息。

## 关于外部流媒体

流媒体来源必须是一个 *https*  URL（*http* （不支持 URL）。

你应该可以粘贴一个指向大多数流行视频流媒体网站的视频 URL。请注意这些平台的服务条款。

要从你本地机器上的视频文件进行串流，最简单的方法是将该视频上传到一个公开的 Google Drive，然后粘贴链接。

另一个选择是使用像 [Vimeo](https://vimeo.com/) , [Livepeer Studio](https://livepeer.studio/) 或 [Serraform](https://serraform.gitbook.io/streaming-docs/guides/decentraland-playback) 在那里，你向提供商支付费用，由其管理所有流媒体基础设施。

这样的视频托管服务提供商。最强大的方式是搭建你自己的服务器，使用免费软件，但需要在像 [Digital Ocean](https://try.digitalocean.com/developerbrand/?_campaign=emea_brand_kw_en_cpc&_adgroup=digitalocean_exact_exact&_keyword=digitalocean&_device=c&_adposition=&_content=conversion&_medium=cpc&_source=bing&msclkid=160bfc160a2a1bab9bbf9933594bd9c5&utm_source=bing&utm_medium=cpc&utm_campaign=emea_brand_kw_en_cpc&utm_term=digitalocean&utm_content=DigitalOcean%20Exact_Exact) 或 [Cloudflare](https://www.cloudflare.com/products/cloudflare-stream/)这样的平台注册托管。你可以部署类似 [Node Media Server](https://github.com/illuspas/Node-Media-Server)的方案，它开箱即用，能提供你所需的大部分功能。

这些选项在不同场景下各有利弊。你应该结合自己的需求、技术水平和预算来评估哪个最适合你。

## 设置 OBS 以成功进行直播

[OBS](https://obsproject.com/) 是一个用于管理串流的流行免费工具。

无论你使用的是场地提供的流密钥还是你自己的 RTMP 服务器，OBS 中的设置都对串流成功至关重要。你应该争取一个稳定、持续的连接。

### 简单的 OBS 设置

建议采用以下简单设置：

* 比特率 2500kbps（所有 Decentraland 场地都可使用）
* 音频比特率 160kbps
* 视频编码器预设：硬件 NVENC
* 音频编码器 AAC
* 最高分辨率：720（任何更高的设置都会在 DCL 中引发问题）
* 帧率 30fps


> [!WARNING]
> **📔 注意**：请确保你在 OBS 中禁用了多轨串流。你可以在 设置 > Stream 中找到它。


### 给新主播的建议

* 尽早进行声音检查对于测试你在场地中的设置至关重要。
* 像流密钥里少了一个数字这样的小错误，最有可能搞砸整个串流。
* 不要超过 720 分辨率或 2500 kbps 的比特率。

## 直播流

你可以通过 [直播流](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/shi-shi-yun-ying/live-streaming/) 功能 [管理工具](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/shi-shi-yun-ying/scene-admin/) 来从摄像头直播，或者共享屏幕。

这种串流方式使用了与玩家之间实时通讯相同的通信架构。它设置简单，而且比从外部来源串流的延迟要低得多。

1. 添加一个 [管理工具](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/shi-shi-yun-ying/scene-admin/) 智能物品到你的场景中，以及一个 [视频播放器](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/video-screen/) 来从摄像头直播，或者共享屏幕。
2. 将你的场景发布到 World 或 Genesis City。
3. 以拥有使用 Admin 工具权限的玩家身份进入场景。
4. 打开 Admin 控制台，选择 **Video** 选项卡，然后选择 **Live** 功能并点击 **Get Stream Key** 按钮。
5. 复制 **Server URL** 和 *Streaming key*\* 到你的串流软件中（例如 OBS）。
6. 按下 **Activate** 按钮开始串流。

除了向场景中添加 Video player 智能物品外，你还可以使用 URL `livekit-video://current-stream` 作为视频源，在场景中播放该流。你仍然需要使用 Admin 工具来获取流密钥。

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

## 视频材质

大多数情况下，你会希望在无光照的 [Basic 材质](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/materials/#unlit-materials)上播放视频，而不是在 PBR 材质上播放。这样会得到更明亮、更清晰的图像，而且性能更好。

```ts
Material.setBasicMaterial(screen, {
	texture: videoTexture,
})
```

通常建议在 Basic 无光照材质上播放视频，因为这样性能更好。不过，如果你想将视频投射到 PBR 材质上，请记住默认属性会让视频看起来比较不透明。你可以通过修改材质的其他属性来增强这一点。下面是一些推荐设置，可让视频更醒目：

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
> **💡 提示**：由于视频是作为纹理添加到材质上的，你还可以尝试材质的其他属性，例如用颜色给它上色，或者添加其他纹理层，例如做出脏屏幕效果。
> 
> 请参阅 [材质](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/materials/) 以了解更多详情。


## 关于视频文件

支持以下文件格式：

* *.mp4*
* *.ogg*
* *.webm*

请注意，视频文件会增加场景的总大小，这会让进入你场景的玩家下载场景所需的时间更长。视频大小也可能让你超出 [场景限制](/docs/creator/content-creator-zh/chang-jing-sdk7/you-hua/scene-limitations/)，因为你每个地块最多可使用 15 MB。我们建议尽可能压缩视频，以减少这个问题。

我们还建议在玩家靠近时，或者玩家执行某个动作触发时再开始播放视频。当场景在远处视野中加载时就开始播放，会在玩家访问相邻场景时不必要地影响性能。

## 开始、暂停和停止视频

要开始播放或暂停视频，请将 `playing` 属性设为 *true* 或 *false*。如果 `playing` 设为 false，视频会在显示的最后一帧处暂停。

你可以通过添加指针事件让屏幕可切换，如下所示：

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

要停止视频并回到第一帧，请将 `位置` 属性设为 0。在下面的示例中，点击视频会使其停止。

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

## 配置视频播放器

以下可选属性可用于设置 `VideoPlayer` 组件：

* `playing`：决定视频当前是否在播放。如果为 false，视频会暂停。


> [!WARNING]
> **📔 注意**：同时可播放的视频数量有限，取决于玩家的画质设置。如果场景播放的视频超过限制，引擎会暂停优先级最低的那些。参见 [性能注意事项](#performance-considerations).


* `playbackRate`：更改视频播放速度。 *1* ，默认情况下。
* `volume`：允许你更改音量。 *1* ，默认情况下。
* `位置`：允许你为视频设置不同的起始位置。它以视频原始开头之后的秒数表示。 *0* 默认为
* `loop`：布尔值，决定视频是连续循环播放，还是只播放一次后停止。 *false* ，默认情况下。

## 播放多个视频

为了避免遇到性能问题，请将同一时间播放的不同视频数量保持在较低水平；引擎只允许有限数量的视频同时播放，具体取决于玩家的画质设置（见 [性能注意事项](#performance-considerations)）。不过，一个场景可以在多个不同屏幕上播放同一视频纹理的多个副本。这不受限制，因为它对性能的影响远小于播放独立的视频。要在多个实体上播放同一个视频，只需将同一个视频纹理对象实例分配给每个屏幕实体的 `材质` 组件即可。

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

请注意，在上面的示例中，只需要创建一个 `VideoPlayer` 组件，它控制两个视频屏幕的状态。在这种情况下，该组件被分配给 `screen1` 实体，但也可以分配给场景中的任何其他实体，不一定是某个屏幕。

## 视频事件

轻松处理视频中的状态变化，以响应视频开始播放、暂停等事件。这可用于例如让动画与视频完美同步，确保它们与视频同时开始。

使用 `videoEventsSystem.registerVideoEventsEntity` 来定义一个函数，每当分配给某个实体的视频状态发生变化时就会运行。每次状态变化时，你的函数都可以检查新状态并做出相应响应。

```ts
import {
	engine,
	Entity,
	VideoPlayer,
	videoEventsSystem,
	VideoState,
} from '@dcl/sdk/ecs'

// ... 创建带有 VideoPlayer 组件、Transform、MeshRenderer.setPlane() 等的 videoPlayerEntity ...

videoEventsSystem.registerVideoEventsEntity(
	videoPlayerEntity,
	function (videoEvent) {
		console.log(
			'视频事件 - 状态: ' +
				videoEvent.state +
				'\n当前偏移量:' +
				videoEvent.currentOffset +
				'\n视频长度:' +
				videoEvent.videoLength
		)

		switch (videoEvent.state) {
			case VideoState.VS_READY:
				console.log('视频事件 - 视频已就绪')
				break
			case VideoState.VS_NONE:
				console.log('视频事件 - 视频处于无状态')
				break
			case VideoState.VS_ERROR:
				console.log('视频事件 - 视频出错')
				break
			case VideoState.VS_SEEKING:
				console.log('视频事件 - 视频正在定位')
				break
			case VideoState.VS_LOADING:
				console.log('视频事件 - 视频正在加载')
				break
			case VideoState.VS_BUFFERING:
				console.log('视频事件 - 视频正在缓冲')
				break
			case VideoState.VS_PLAYING:
				console.log('视频事件 - 视频已开始播放')
				break
			case VideoState.VS_PAUSED:
				console.log('视频事件 - 视频已暂停')
				break
		}
	}
)
```

作为函数输入传入的 videoEvent 对象包含以下属性：

* `currentOffset` (*数字*）：视频上 `位置` 属性的当前值。此值显示的是从视频原始开始位置起经过的秒数。 *-1* 如果视频尚未开始播放。
* `状态`：新的视频状态，以 `VideoState` 枚举值表示。该枚举可能包含以下值：
  * `VideoState.VS_READY`
  * `VideoState.VS_NONE`
  * `VideoState.VS_ERROR`
  * `VideoState.VS_SEEKING`
  * `VideoState.VS_LOADING`
  * `VideoState.VS_BUFFERING`
  * `VideoState.VS_PLAYING`
  * `VideoState.VS_PAUSED`
* `视频长度` (*数字* ）：整个视频的长度（秒）。 *-1* 如果长度未知则为。
* `timestamp` ( *数字*）：一个 *lamport* timestamp，每当视频改变状态时都会递增。
* `tickNumber` (*数字*）：事件发生的时间，以场景开始运行以来的 tick 计数表示。

如果你在一个已经有回调的实体上重新注册回调（例如，想更改状态变化时的行为），新的回调会替换旧的回调，并保留最后报告的状态。这意味着，对于之前已经报告给旧回调的状态，新的回调不会再次被调用。

### 检查某个实体是否已注册

使用 `videoEventsSystem.hasVideoEventsEntity()` 来检查某个实体是否已经注册了视频事件回调。

```ts
if (videoEventsSystem.hasVideoEventsEntity(videoPlayerEntity)) {
	console.log('这个实体已经有一个视频事件监听器')
}
```

### 最新的视频事件

使用 `videoEventsSystem.getVideoState()`查询视频的最后一次状态变化。这会返回该实体最新的 `VideoEvent` 值；如果还没有报告过任何事件，则返回 `undefined` 。你可以在系统中调用它，以便在不注册回调的情况下每一帧轮询视频状态。

```ts
function mySystem() {
	const latestVideoEvent = videoEventsSystem.getVideoState(videoPlayerEntity)
	if (!latestVideoEvent) return

	console.log(`状态: ${latestVideoEvent.state}
    \n当前偏移量: ${latestVideoEvent.currentOffset}
    \n视频长度: ${latestVideoEvent.videoLength}`)
}
```

## 视频上的 Alpha 遮罩

要实现非矩形的视频屏幕，一个巧妙的方法是在平面上叠加一层 alpha 纹理。你可以把平面的一部分裁剪成任何你想要的形状。

使用下图可将你的视频裁剪为圆形，并保留透明角落。

![](/docs/_remote/1f517d241f0753831e85b88c.webp)

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

![](/docs/_remote/9b242e1d8862ee66017fbceb.webp)


> [!WARNING]
> **📔 注意**：在早期版本中， `alphaTexture` 属性只存在于 PBR 材质中。当前它只适用于 Basic 材质。


## 在 glTF 模型上播放视频

你可以使用 *glTF* 模型来播放视频，方法是使用 [GltfNodeModifiers](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/materials/#modify-gltf-materials) 组件遍历所有玩家。参见 [修改 glTF 材质](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/materials/#modify-gltf-materials) 以了解更多详情。

这让你可以在任何形状上播放视频，而不仅仅是平面。例如，你可以在弯曲的屏幕上播放视频，甚至可以播放到 NPC 的整个身体上。

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

视频的映射将遵循模型所使用的原始 UV 映射。这意味着，如果模型有一个纹理映射到了模型的特定部分，那么视频也会映射到同一部分。

你也可以使用 `GltfNodeModifiers` 组件，只在模型内部的某个特定网格上播放视频。例如，即使模型覆盖了整栋建筑，你也可以只在建筑的某面墙上播放。参见 [修改 glTF 材质](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/materials/#modify-gltf-materials) 以了解更多详情。

## 空间音频

默认情况下，来自 `VideoPlayer` 组件的视频是全局的，这意味着在整个场景中，它都会以一致的音量被听到。如果玩家走出场景，他们将完全听不到该串流。

要让音频具有空间效果，请将 `spatial` 属性设为 *true*.

```ts
VideoPlayer.create(entity, {
	src: 'https://player.vimeo.com/progressive_redirect/playback/1145666916/rendition/540p/file.mp4%20%28540p%29.mp4?loc=external&signature=db1cd6946851313cb8f7be60d1f6c30af0902bcc46fdae0ba2a06e5fdf44c329',
	playing: true,
	spatial: true,
})
```

视频现在会从拥有该 `VideoPlayer` 组件的实体位置发出，并且玩家越接近，它的声音就会越大。

使用以下属性控制空间音频：

* `spatialMinDistance`：音频开始变为空间音频的最小距离。如果玩家更近，音频将以全音量播放。 *0* ，默认情况下。
* `spatialMaxDistance`：音频可被听到的最大距离。如果玩家更远，音频将以 0 音量播放。 *60* 默认情况下

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
> **📔 注意**：有些视频格式不支持空间音频。请确保串流已编码为 *mp4*, *m4a*，或 *mov*.


## 音频分析

您可以从一个音轨中读取实时振幅和频率数据 `VideoPlayer` 实体，以驱动与视频配乐同步的响应式视觉效果。参见 [音频分析](/docs/creator/content-creator-zh/chang-jing-sdk7/mei-ti/audio-analysis/).


> [!NOTE]
> **💡 提示**：如需查看在 GLTF 模型而非基本几何体上播放的视频，请参见 [`74,-8-gltfnodemodifier`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/74,-8-gltfnodemodifier) 测试场景，它使用 `GltfNodeModifiers` 视频纹理。



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/mei-ti/video-playing.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
