> 从场景中播放的音频读取实时振幅和频率数据，以驱动响应式视觉效果。

# 音频分析

该 `AudioAnalysis` 组件会从场景中正在播放的音频源读取实时数据，因此你可以用音乐来驱动视觉效果。每一帧，它会报告一个整体 **振幅** 值，以及 **8 个频段** （从低到高），你可以用它们来缩放、着色或为实体添加动画。

常见用途：

* 随着低音跳动的立方体。
* 随着节拍脉动的灯光。
* 均衡器风格的柱状可视化器。
* 与音乐同步反应的材质或缩放道具。

`AudioAnalysis` 是一个只读流——你的场景只会消费这些值，由运行时填充。它适用于通过以下方式播放音频的实体： [AudioSource](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/sounds/)，一个 [AudioStream](/docs/creator/content-creator-zh/chang-jing-sdk7/mei-ti/audio-streaming/)，或者某个 [VideoPlayer](/docs/creator/content-creator-zh/chang-jing-sdk7/mei-ti/video-playing/).


> [!WARNING]
> **📔 注意**: `AudioAnalysis` 目前仅受官方 Decentraland 桌面应用支持。在其他客户端上，该组件的值不会被填充，所以请确保即使数据始终没有到达，你的场景也能正常工作。


## 最小示例

以下场景会播放一个声音，并将 `AudioAnalysis` 连接到同一个实体，并使用音频的振幅在每一帧缩放一个立方体。

```ts
import {
  engine,
  Transform,
  MeshRenderer,
  AudioSource,
  AudioAnalysis,
  AudioAnalysisView,
} from "@dcl/sdk/ecs";
import { Vector3 } from "@dcl/sdk/math";

export function main() {
  // 在实体上播放声音
  const audioEntity = engine.addEntity();
  Transform.create(audioEntity);
  AudioSource.create(audioEntity, {
    audioClipUrl: "sounds/music.mp3",
    playing: true,
    loop: true,
  });

  // 将 AudioAnalysis 连接到同一个实体以开始接收分析数据
  AudioAnalysis.createAudioAnalysis(audioEntity);

  // 一个可复用的视图对象，组件会在每一帧写入其中
  const analysis: AudioAnalysisView = {
    amplitude: 0,
    bands: new Array<number>(8),
  };

  // 一个会随音频振幅脉动的立方体
  const cube = engine.addEntity();
  MeshRenderer.setBox(cube);
  Transform.create(cube, { position: Vector3.create(8, 1, 8) });

  // 每一帧，读取最新的分析值并缩放立方体
  engine.addSystem(() => {
    AudioAnalysis.readIntoView(audioEntity, analysis);
    const s = 1 + analysis.amplitude * 10;
    Transform.getMutable(cube).scale = Vector3.create(s, s, s);
  });
}
```

关键调用如下：

1. `AudioAnalysis.createAudioAnalysis(entity)` — 将该组件附加到拥有以下内容的实体上： `AudioSource`, `AudioStream`，或者 `VideoPlayer`。默认使用对数模式（参见 [模式](#modes)).
2. `AudioAnalysis.readIntoView(entity, analysis)` — 将最新的 `振幅` 和 8 个 `频段` 的值复制到你提供的视图对象中。


> [!WARNING]
> **📔 注意**：在调用 `频段` 数组之前，始终预先分配一个包含 **8 个元素** 的 `readIntoView`。该组件不会为你调整数组大小。


## 每帧读取数据

`AudioAnalysis` 设计为由系统每帧读取一次。推荐的模式是：

1. 只分配一个 `AudioAnalysisView` 对象一次。
2. 在系统中，调用 `readIntoView` 来刷新它。
3. 直接使用这些值，或在多个系统之间共享该视图。

```ts
const analysis: AudioAnalysisView = {
  amplitude: 0,
  bands: new Array<number>(8),
};

engine.addSystem(() => {
  AudioAnalysis.readIntoView(audioEntity, analysis);
  // analysis.amplitude 和 analysis.bands[0..7] 现在保存着最新值
});
```

如果被分析的实体可能尚未拥有一个 `AudioAnalysis` 组件（例如，它稍后才创建或被动态移除），请使用 `tryReadIntoView` 代替。它会返回 `false` 当组件缺失时，以及 `true` 当值已写入时。

```ts
engine.addSystem(() => {
  if (!AudioAnalysis.tryReadIntoView(audioEntity, analysis)) return;
  // 这里可以安全使用 analysis
});
```


> [!NOTE]
> **💡 提示**：调用 `readIntoView` 一次，并让多个系统共享同一个视图对象，比反复读取组件更省资源。


## 对特定频段作出反应

这 8 个频段覆盖了从低到高的可听频谱。 `bands[0]` 是最低的（低音），而 `bands[7]` 是最高的（高音）。你可以用不同频段驱动不同实体，构建经典均衡器。

```ts
import {
  engine,
  Transform,
  MeshRenderer,
  Material,
  AudioSource,
  AudioAnalysis,
  AudioAnalysisView,
  Schemas,
} from "@dcl/sdk/ecs";
import { Color4, Vector3 } from "@dcl/sdk/math";

// 自定义组件，用于标记每个柱条并记录它跟踪哪个频段
const VisualBar = engine.defineComponent("visual-bar", {
  index: Schemas.Number,
});

const BARS_HEIGHT = 12;

export function main() {
  const audioEntity = engine.addEntity();
  Transform.create(audioEntity);
  AudioSource.create(audioEntity, {
    audioClipUrl: "sounds/music.mp3",
    playing: true,
    loop: true,
  });
  AudioAnalysis.createAudioAnalysis(audioEntity);

  const analysis: AudioAnalysisView = {
    amplitude: 0,
    bands: new Array<number>(8),
  };

  // 创建 8 个柱条，每个频段一个
  for (let i = 0; i < 8; i++) {
    const bar = engine.addEntity();
    VisualBar.create(bar, { index: i });
    Transform.create(bar, { position: Vector3.create(4 + i, 0, 8) });
    MeshRenderer.setBox(bar);
    Material.setPbrMaterial(bar, { albedoColor: Color4.Yellow() });
  }

  // 每帧读取一次
  engine.addSystem(() => {
    AudioAnalysis.readIntoView(audioEntity, analysis);
  });

  // 按照各自的频段缩放每个柱条
  engine.addSystem(() => {
    for (const [entity] of engine.getEntitiesWith(VisualBar, Transform)) {
      const index = VisualBar.get(entity).index;
      const transform = Transform.getMutable(entity);
      transform.scale = Vector3.create(
        1,
        analysis.bands[index] * BARS_HEIGHT,
        1
      );
    }
  });
}
```

这与以下示例中使用的模式相同： [音频可视化示例场景](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/88,-10-audio-visualization)，只是删去了不必要的部分。

## 模式

`AudioAnalysis` 支持两种分析模式，在创建组件时设置：

* `PBAudioAnalysisMode.MODE_LOGARITHMIC` （默认）：数值会按对数曲线缩放，这更接近人耳对音量变化的感知。最适合视觉响应。
* `PBAudioAnalysisMode.MODE_RAW`：未处理的振幅和 FFT 频段值。如果你想自行应用缩放，请使用此模式。

```ts
import { AudioAnalysis, PBAudioAnalysisMode } from "@dcl/sdk/ecs";

// 使用原始值
AudioAnalysis.createAudioAnalysis(audioEntity, PBAudioAnalysisMode.MODE_RAW);
```

### 调整对数模式

在对数模式下，你可以传入两个可选的增益乘数：

* `amplitudeGain` — 作用于整体振幅的乘数。默认值为 `5`.
* `bandsGain` — 作用于全部 8 个频段的乘数。默认值为 `0.05`.

更高的增益会使数值更敏感；更低的增益会让它们更微妙。

```ts
// 更强的振幅响应，更柔和的频段响应
AudioAnalysis.createAudioAnalysis(
  audioEntity,
  PBAudioAnalysisMode.MODE_LOGARITHMIC,
  10, // amplitudeGain
  0.03 // bandsGain
);
```


> [!WARNING]
> **📔 注意**: `amplitudeGain` 和 `bandsGain` 仅在对数模式下生效。在原始模式下会被忽略。


## 替换现有组件

`createAudioAnalysis` 如果实体已经拥有一个，则会失败 `AudioAnalysis` 组件。若要在运行时切换模式或增益，请使用 `createOrReplaceAudioAnalysis`，其签名相同。

```ts
// 在场景运行中切换到原始模式
AudioAnalysis.createOrReplaceAudioAnalysis(
  audioEntity,
  PBAudioAnalysisMode.MODE_RAW
);
```

## 组件参考

### `AudioAnalysis.createAudioAnalysis(entity, mode?, amplitudeGain?, bandsGain?)`

将一个 `AudioAnalysis` 组件附加到 `实体`。如果已存在组件则会失败。

* `实体` (`实体`）：拥有以下内容的实体： `AudioSource`, `AudioStream`，或者 `VideoPlayer` 你想要分析的对象。
* `模式` (`PBAudioAnalysisMode`，可选）： `MODE_LOGARITHMIC` （默认）或 `MODE_RAW`.
* `amplitudeGain` (`数字`，可选）：对数模式下的振幅乘数。默认值 `5`.
* `bandsGain` (`数字`，可选）：对数模式下的频段乘数。默认值 `0.05`.

### `AudioAnalysis.createOrReplaceAudioAnalysis(entity, mode?, amplitudeGain?, bandsGain?)`

与上面相同，但会替换实体上现有的 `AudioAnalysis` 组件，而不是失败。

### `AudioAnalysis.readIntoView(entity, out)`

将最新值读取到 `out`。如果 `实体` 没有 `AudioAnalysis` 组件，则抛出异常。

* `实体` (`实体`）：带有该组件的实体。
* `out` (`AudioAnalysisView`）：你提供的视图对象。必须已 `频段` 预先分配 8 个数字。

### `AudioAnalysis.tryReadIntoView(entity, out): boolean`

与 `readIntoView`相同，但当实体没有 `false` 组件时不会抛出异常，而是返回 `AudioAnalysis` 当值已写入时返回。 `true` A plain object you allocate to receive the analysis data:

### `AudioAnalysisView`

你分配的一个普通对象，用于接收分析数据：

```ts
type AudioAnalysisView = {
  amplitude: number; // 整体信号强度
  bands: number[]; // 8 个频段，从低（0）到高（7）
};
```

## 注意事项与限制

* **8 个频段，固定不变。** 频段数量固定为 8。没有 API 可以请求更多或更少的频段。
* **每次分析对应一个音频源。** 每个 `AudioAnalysis` 组件都会分析其所附加实体的音频。若要分析多个源，请分别附加 `AudioAnalysis` 到每个源上。
* **视频配乐也会被分析。** `AudioAnalysis` 读取某个 [`VideoPlayer`](/docs/creator/content-creator-zh/chang-jing-sdk7/mei-ti/video-playing/) 的音频，方式与读取 `AudioSource`相同，适用于渐进式视频文件（例如 `.mp4`）以及实时 HLS 流（`.m3u8`）。将 `AudioAnalysis` 附加到持有 `VideoPlayer`.
* **暂停的音频不会报告更新。** 当底层音频停止或暂停时，组件值将停止变化。你上次读取的值会保留在视图对象中，直到音频再次播放。
* **开销很小，但并非零成本。** 每帧分析的设计目标是尽可能低开销（桌面端每个源不足一毫秒）。如果你不需要这些数据，请避免附加 `AudioAnalysis` 到太多源上。可视化器不可见时，请移除该组件。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/mei-ti/audio-analysis.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
