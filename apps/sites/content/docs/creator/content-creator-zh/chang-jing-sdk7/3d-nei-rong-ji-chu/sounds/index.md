> 了解如何向场景中添加声音。

# 声音

声音是为玩家操作和事件提供反馈的绝佳方式，背景音效也可以为你的场景提供更多上下文，并提升玩家的沉浸感。


> [!WARNING]
> **📔 注意**：请记住，只有站在组成生成声音所在场景的地块内的玩家才能听到声音，即使他们原本处于可听范围内。玩家也可以在设置中选择关闭所有声音。


支持的音频格式因浏览器而异，但建议使用 *.mp3*.

*.wav* 文件也受支持，但一般不推荐，因为它们要大得多。

## 播放声音

播放声音最简单的方法是添加一个 **音频源** 组件到 [Creator Hub 中的场景编辑器](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/ru-men/about-editor/) 并将其设置为 **开始播放** 和 **循环**。参见 [添加组件](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/gou-jian/components/#add-components).

![](/docs/_remote/341e53c596dc617859805f3b.webp)

你也可以通过以下方式以无代码方式触发声音播放： **操作**，参见 [让任何物品变智能](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/make-any-item-smart/).

要通过代码播放声音，请使用 `AudioSource.playSound` 函数。

```ts
// 创建实体
const sourceEntity = engine.addEntity()
Transform.create(sourceEntity, { position: Vector3.create(8, 1, 8) })

// 播放声音
AudioSource.playSound(sourceEntity, 'assets/sounds/sound-effect.mp3')
```


> [!WARNING]
> **📔 注意**：每个带有 `AudioSource` 还需要一个 `Transform`，包括全局声音。没有它，任何内容都不会播放，也不会记录错误，因此这很难被发现。放置在场景编辑器中的物品始终都有一个 `Transform`，所以这只会影响你在代码中创建的实体。


声音文件必须位于项目文件夹内。在上面的示例中，音频文件位于 `assets/sounds` 文件夹中，它位于场景项目文件夹的根目录。


> [!WARNING]
> **📔 注意**： `AudioSource` 组件必须通过以下方式导入
> 
> > `import { AudioSource } from "@dcl/sdk/ecs"`
> 
> 参见 [导入](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/coding-scenes/#imports) ，了解如何轻松处理这些内容。


该 `AudioSource.playSound()` 函数接受以下参数：

* `entity`：要将声音应用到哪个实体。声音将从该实体的位置发出，这意味着玩家越接近它，声音就越大。
* `src`：声音文件在项目中的位置。


> [!NOTE]
> **💡 提示**：为了更清晰，我们建议将声音文件单独放在场景中的一个 `assets/sounds` 文件夹中。


* `resetCursor`: *（可选）* 如果为 true，声音总是从头开始。否则它会从当前光标位置继续播放。适用于暂停和恢复。

`playSound()` 会创建 `AudioSource` 组件，如果实体还没有该组件的话。如果它已经有了， `playSound()` 并保留其他值，例如 `volume`, `pitch`, `loop` 和 `global`，因此你可以只设置一次，然后在每次想播放声音时调用 `playSound()` 。每次你想播放声音时都可以这么做。再次使用同一文件调用时会重新播放声音。它始终返回 `true`.

另一种播放声音的方法是在实体上手动创建一个 `AudioSource` 组件。使用这种方法可以更精细地控制声音，例如让它循环播放或设置音量。

```ts
// 创建实体
const sourceEntity = engine.addEntity()
Transform.create(sourceEntity, { position: Vector3.create(8, 1, 8) })

// 创建 AudioSource 组件
AudioSource.create(sourceEntity, {
	audioClipUrl: 'sounds/sound-effect.mp3',
	loop: true,
	playing: true,
})
```

可设置以下属性：

* `audioClipUrl`：声音文件在项目中的位置。
* `playing`：如果为 true，声音会开始播放。你可以先创建一个将 `playing` 设置为 false 的声音，然后在稍后将其设为 true。
* `volume`: *（可选）* 声音文件的音量。默认值为 1，即最大音量。
* `pitch`: *（可选）* 调整声音的音高。默认值为 1；更低会使声音更低沉，更高会使声音更尖锐。


> [!NOTE]
> **💡 提示**：为了防止游戏中的音效过于重复，每次播放时对音高做一些轻微的随机变化会很有用。


* `currentTime`: *（可选）* 默认值为 0。设置此值可避免从声音文件的开头开始播放。

每个实体只能有一个 `AudioSource` 组件，它一次只能播放一个片段。这个限制可以很容易地通过在播放新声音时修改音频源来克服，或者通过包含多个不可见的子实体，每个子实体都有自己的声音。


> [!WARNING]
> **📔 注意**：声音会在每个玩家的本地实例中播放。除非附近其他玩家的本地场景也明确播放相同声音，否则他们不会听到这些声音。


### 预加载声音

如果某个实体使用了声音，但在场景运行时不会立即播放，那么下载可能需要一些时间。可以通过使用 `AssetLoad` 组件，使其在场景运行时可用。

```ts
import { AssetLoad } from "@dcl/sdk/ecs"

AssetLoad.create(engine.RootEntity, {
  assets: [
    "assets/scene/bundle1/explosionSound.mp3",
  ],
})
```

更多信息请查看 [预加载资源](/docs/creator/content-creator-zh/chang-jing-sdk7/you-hua/pre-load-resources/) 文档。

## 停止声音

要停止实体播放其声音，请使用 `AudioSource.stopSound()` 函数。你只需要指定实体，因为每个实体只有一个 `AudioSource` 组件，并且每个 `AudioSource` 组件一次只能播放一个文件。

```ts
AudioSource.stopSound(sourceEntity)
```

`stopSound()` 返回 `false` 如果实体没有 `AudioSource` 组件，使其在场景运行时可用。

另一种停止声音的方法是将 `playing` 属性设为 false。

```ts
// 创建实体
const sourceEntity = engine.addEntity()

// 创建 AudioSource 组件
AudioSource.create(sourceEntity, {
	audioClipUrl: 'sounds/explosion.mp3',
	playing: true,
})

// 定义一个简单函数
function stopSound(entity: Entity) {
	// 获取音频源组件的可变版本
	const audioSource = AudioSource.getMutable(entity)

	// 修改其 playing 值
	audioSource.playing = false
}

// 调用函数
stopSound(sourceEntity)
```

## 检测声音何时结束

当一个非循环声音自行播放结束时，引擎会将 `playing` 的属性 `AudioSource` 组件恢复为 *false*。你的场景代码可以读取该值来知道声音何时结束，例如在其后立即串联另一个声音或动作。

```ts
let wasPlaying = false

engine.addSystem(() => {
	const audio = AudioSource.get(sourceEntity)
	const isPlaying = audio.playing ?? false

	if (wasPlaying && !isPlaying) {
		console.log('sound finished playing')
		// 在这里响应：播放另一个声音、推进序列等。
	}

	wasPlaying = isPlaying
})
```


> [!WARNING]
> **📔 注意**：当每帧轮询该组件时，始终使用 `AudioSource.get()` （只读）。使用 `AudioSource.getMutable()` 会将该组件标记为每帧都发生变化，从而导致不必要的同步工作。
> 
> 该 `playing` 属性只会在声音自行结束时由引擎翻转。如果场景显式停止声音（通过 `AudioSource.stopSound()` 或通过将 `playing` 设为 *false*），该变化来自你自己的代码；而循环声音会一直播放直到被停止，因此它们不会自行翻转该属性。


或者，使用 `audioEventsSystem.registerAudioEventsEntity` 来定义一个函数，每当实体音频的播放状态发生变化时都会运行，就像 [视频事件](/docs/creator/content-creator-zh/chang-jing-sdk7/mei-ti/video-playing/#video-events) 用于视频一样。该函数会接收一个带有 `state` 字段的事件，使用 `MediaState` 枚举。

```ts
import { engine, AudioSource, audioEventsSystem, MediaState } from '@dcl/sdk/ecs'

// ... 使用 AudioSource 组件创建 sourceEntity ...

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

从 `MS_PLAYING` 设为 `MS_READY` 表示声音已停止播放。完整的 `MediaState` 枚举还包括 `MS_NONE`, `MS_LOADING`, `MS_PAUSED`, `MS_BUFFERING`, `MS_SEEKING` 和 `MS_ERROR`，并且每个事件都携带一个 `时间戳` ，用于记录状态更改发生的时间。你也可以随时通过以下方式查询最新报告的状态： `audioEventsSystem.getAudioState(entity)`，可以使用以下方式注销回调： `audioEventsSystem.removeAudioEventsEntity(entity)`，或者通过以下方式检查某个实体是否已注册： `audioEventsSystem.hasAudioEventsEntity(entity)`。同一系统也适用于带有 [`AudioStream`](/docs/creator/content-creator-zh/chang-jing-sdk7/mei-ti/audio-streaming/) 组件，使其在场景运行时可用。


> [!WARNING]
> **📔 注意**：这些功能仅受桌面客户端支持。


## 循环播放

要让声音持续循环播放，请将 `loop` 字段 `AudioSource` 组件设置为 *true* ，然后再开始播放。

```ts
// 创建实体
const sourceEntity = engine.addEntity()

// 创建 AudioSource 组件
AudioSource.create(sourceEntity, {
	audioClipUrl: 'sounds/sound-effect.mp3',
	playing: true,
	loop: true,
})
```

循环播放声音对于添加背景音乐或其他背景音效特别有用。

## 设置音量

你可以设置 `volume` 的属性 `AudioSource` 组件来更改声音的音量。

音量以一个数值表示，范围从 *0* 设为 *1*.

```ts
// 创建实体
const sourceEntity = engine.addEntity()

// 创建 AudioSource 组件
AudioSource.create(sourceEntity, {
	audioClipUrl: 'sounds/sound-effect.mp3',
	playing: true,
	volume: 0.5,
})
```


> [!WARNING]
> **📔 注意**：当然，声音的音量也会受到玩家与音频源之间距离的影响。玩家走远时，音量会更低。


## 全局声音

默认情况下，来自一个 `AudioSource` 的所有声音都是有空间位置的。这意味着它们看起来是从 `Transform` 组件的位置发出，并且玩家越靠近，声音就越大。但你也可以将声音配置为全局声音，这样无论玩家站在哪里，音量都保持恒定。这非常适合用于背景音乐、通知声音以及其他非位置性声音。


> [!WARNING]
> **📔 注意**：全局声音是 DCL 2.0 桌面客户端才支持的功能。


要将声音设为全局，请将 `global` 属性设为 *true*.

```ts
AudioSource.create(sourceEntity, {
	audioClipUrl: 'sounds/music.mp3',
	playing: true,
	global: true,
})
```

全局声音仍然需要一个 `Transform` 放在其实体上，尽管它的位置不再影响玩家听到的内容。任何位置都可以。

## 播放声音片段

要播放较长声音文件的某个片段，请使用 `playSoundSegment()` ，位于 SDK Utils 库中。参见 [SDK7 Utils](https://github.com/decentraland/sdk7-utils).

你也可以通过显式设置 `currentTime` 在某个 `AudioSource` 组件上的该属性，然后等待一段时间后将其停止。

## 音频流

参见 [音频流](/docs/creator/content-creator-zh/chang-jing-sdk7/mei-ti/audio-streaming/) ，了解如何播放来自外部来源的实时音频流。

## 音频分析

参见 [音频分析](/docs/creator/content-creator-zh/chang-jing-sdk7/mei-ti/audio-analysis/) ，了解如何从一个 `AudioSource` 中读取实时振幅和频率数据，并据此驱动响应式视觉效果，例如随着低音脉动的立方体或均衡器风格的条形可视化器。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/sounds.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
