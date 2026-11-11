> 在场景中播放实时音频流。

# 音频流

你可以从 URL 传输音频。这可用于直接从网络电台播放音乐，或将会议直播流到你的场景中。


> [!NOTE]
> **💡 提示**：在 [Creator Hub 中的场景编辑器](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/ru-men/about-editor/)，你可以使用一个 **音频流** [智能物品](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/smart-items/) 用于以无代码方式实现这一点。


源中的音频必须采用以下格式之一： `.mp3`, `ogg`，或 `aac`。源还必须是一个 *https* URL（*http* URL 不受支持），并且该来源应具有 [CORS 策略（跨源资源共享）](https://en.wikipedia.org/wiki/Cross-origin_resource_sharing) 允许外部访问它的。如果不是这样，你可能需要搭建一台服务器作为代理，并以有效的方式公开该流。


> [!WARNING]
> **📔 注意**：若要在你的场景中播放预录制的声音，请参见 [声音](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/sounds/).


要将音频流添加到你的场景中，只需添加一个 `AudioStream` 组件到实体：

```ts
const streamEntity = engine.addEntity()

AudioStream.create(streamEntity, {
	url: 'https://icecast.ravepartyradio.org/ravepartyradio-192.mp3',
	playing: true,
	volume: 0.8,
})
```


> [!WARNING]
> **📔 注意**：流式声音没有位置感，在整个场景中都会以恒定音量播放。如果玩家离开场景，他们将完全听不到该流。


设置……的音量 `AudioStream` 组件，方法是更改其 `volume` 属性定义。

切换 `AudioStream` 组件的开关状态，方法是设置其 `playing` 属性为 *true* 或 *false*.


> [!NOTE]
> **📔 注意**：并非所有流媒体服务都允许你在其网站之外播放音频。以下是一些在 Decentraland 中可用的示例：
> 
> ```ts
> DELTA = "https://cdn.instream.audio/:9069/stream?_=171cd6c2b6e"
> GRAFFITI = "https://n07.radiojar.com/2qm1fc5kb.m4a?1617129761=&rj-tok=AAABeIR7VqwAilDFeUM39SDjmw&rj-ttl=5"
> ISLA NEGRA = "https://radioislanegra.org/listen/up/basic.aac"
> ```
> 


## 流状态

你可以使用以下任一方式监控音频流的状态： `audioEventsSystem` （基于回调，推荐）或通过轮询使用 `AudioStream.getAudioState()`.

### 使用 audioEventsSystem（推荐）

注册一个仅在流状态发生变化时触发的回调。这与以下功能使用的是同一系统： `AudioSource` 实体（参见 [检测声音何时结束](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/sounds/#detect-when-a-sound-finishes)).

```ts
import { engine, AudioStream, MediaState } from '@dcl/sdk/ecs'
import { audioEventsSystem } from '@dcl/sdk/ecs'

export function main() {
	const entity = engine.addEntity()

	AudioStream.create(entity, {
		playing: true,
		volume: 1,
		url: 'https://audio-edge-es6pf.mia.g.radiomast.io/ref-128k-mp3-stereo',
	})

	audioEventsSystem.registerAudioEventsEntity(entity, (event) => {
		console.log('流状态：', event.state)

		if (event.state === MediaState.MS_ERROR) {
			// 尝试重新连接
		}
	})
}
```

### 使用 getAudioState 轮询

你也可以使用以下方式在每一帧轮询流的状态： `AudioStream.getAudioState()`。这会返回一个 `PBAudioEvent` 对象（或 `undefined` 如果尚未报告任何状态，则为），其中包含一个 `state` 字段和一个 `timestamp` 字段。 `state` 字段是以下项的一个值： `MediaState` 枚举中使用：

* `MS_NONE`
* `MS_LOADING`
* `MS_READY`
* `MS_PLAYING`
* `MS_PAUSED`
* `MS_BUFFERING`
* `MS_SEEKING`
* `MS_ERROR`

```ts
export function main() {
	const entity = engine.addEntity()

	AudioStream.create(entity, {
		playing: true,
		volume: 1,
		url: 'https://audio-edge-es6pf.mia.g.radiomast.io/ref-128k-mp3-stereo',
	})

	let lastState: MediaState | undefined = undefined
	engine.addSystem(() => {
		const currentState = AudioStream.getAudioState(entity)?.state
		if (lastState !== currentState) {
			console.log('流状态：', currentState)
			lastState = currentState

			if (currentState == MediaState.MS_ERROR) {
				// 尝试重新连接
			}
		}
	})
}
```

## 空间音频

默认情况下，来自一个 `AudioStream` 组件的视频是全局的，这意味着在整个场景中都能以一致的音量听到。如果玩家离开场景，他们将完全听不到该串流。

要使音频具有空间效果，请将 `空间` 属性为 *true*.

```ts
AudioStream.create(entity, {
	url: 'https://radioislanegra.org/listen/up/stream',
    playing: true,
	spatial: true,
})
```

现在，音频将从拥有该 `AudioStream` 组件的实体位置发出，并且当玩家靠近时音量会更大。

使用以下属性控制空间音频：

* `spatialMinDistance`：音频开始具有空间效果的最小距离。如果玩家更近，则音频会以全音量播放。 *0* 默认情况下。
* `spatialMaxDistance`：音频可被听到的最大距离。如果玩家距离更远，则音频音量为 0。 *60* 默认情况下

```ts
const audioStreamEntity = engine.addEntity();

Transform.create(audioStreamEntity, {
    position: Vector3.create(8, 0, 8),
});

AudioStream.create(audioStreamEntity, {
    url: 'https://radioislanegra.org/listen/up/stream',
    playing: true,
    volume: 1.0,
    spatial: true,
    spatialMinDistance: 5,
    spatialMaxDistance: 10
});
```


> [!WARNING]
> **📔 注意**：某些音频格式不支持空间音频。请确保流音频编码为 *mp3*, *AAC-LC* 或 *FLAC*.


## 音频分析

你可以从一个 `AudioStream` 实体读取实时振幅和频率数据，以驱动均衡器或随节拍脉动的灯光等响应式视觉效果。参见 [音频分析](/docs/creator/content-creator-zh/chang-jing-sdk7/mei-ti/audio-analysis/).


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/mei-ti/audio-streaming.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
