> 更改天空盒时间，用您自己的纹理替换天空，或者重新着色天空、云朵和星星，并控制太阳和雾

# 天空盒控制

你可以更改玩家站在你的场景中时看到天空盒的方式，这也会影响全局照明的色调和方向。

Decentraland 中的天空遵循默认的昼夜循环，完成一次需要 2 小时，因此每个真实日有 12 个完整循环。如果场景没有强制使用固定的时段，那么玩家也可以通过更改其 UI 中的滑块来切换到特定的时段。

每当玩家进入一个具有不同一天中的时间的场景，或者场景动态更改一天中的时间时，天空盒都会在几秒内平滑过渡到这个新值。

## 固定的一天中的时间

你可以为你的场景设置固定的一天中的时间。所有玩家都会以这个时段看到场景，天空盒不会遵循昼夜循环。

在 Creator Hub 中，打开场景设置并点击 **设置** 选项卡，找到 **天空盒** 部分。取消勾选 **自动** 选项，然后设置你想要的时间。

![](/docs/_remote/6242ab0073574f8cc2c9e7d9.webp)

你也可以在场景代码中设置天空盒的一天中的时间。为此，请在你的 `scene.json` 的根级别：

```json
 "skyboxConfig": {
    "fixedTime": 36000
  }
```

这个数字表示自一天开始以来的秒数，范围从 0（表示 *00:00*）到 86400（表示 *24:00*）。任何大于 86400 的数字也会被解释为午夜。

下面是一些有效值的更多示例：

* 0 秒 => *00:00*
* 21600 秒 => *06:00*
* 43200 秒 => *12:00*
* 64800 秒 => *18:00*
* 86400 秒 => *24:00*

## 读取一天中的时间

你可以在场景代码中使用 `getWorldTime()` 函数读取一天中的时间。

```ts
import { getWorldTime } from '~system/Runtime'

executeTask(async () => {
  let time = await getWorldTime({})
  console.log(time.seconds)
})
```

该函数返回一个介于 0 和 86400 之间的数字，其中 0 表示午夜，86400 表示 24:00。如果场景动态更改了一天中的时间，或者玩家在 UI 中更改了一天中的时间，该值会更新。否则，它返回相对于默认昼夜循环的值。

## 动态更改一天中的时间

你可以使用 `SkyboxTime` 组件动态更改一天中的时间。该组件只能添加到场景的根实体 `engine.RootEntity`.

```ts
import { SkyboxTime } from '@dcl/sdk/ecs'

function main() {
  SkyboxTime.create(engine.RootEntity, { fixedTime: 36000 })
}
```

该 `fixedTime` 属性是一个介于 0 和 86400 之间的数字，其中 0 表示午夜，86400 表示 24:00。任何大于 86400 的数字也会被解释为午夜。

每当添加、移除该组件，或者 `fixedTime` 属性被更改时，天空盒的一天中的时间都会在几秒内平滑过渡到这个新值。玩家走出或进入场景时也会发生同样的情况。当天空盒的一天中的时间固定后，天空盒将不再跟随其昼夜循环的进度，玩家也无法通过 UI 更改一天中的时间。

默认情况下，过渡总是按正向发生，但你可以通过设置 `transitionMode` 属性设置为 `TransitionMode.TM_FORWARD` 或 `TransitionMode.TM_BACKWARD`.

```ts
import { SkyboxTime, TransitionMode } from '@dcl/sdk/ecs'

function main() {
  SkyboxTime.create(engine.RootEntity, { fixedTime: 36000, transitionMode: TransitionMode.TM_BACKWARD })
}
```

### 选择更短的过渡

从来不会突然跳变：每次更改都会平滑经过中间的所有时间点。 `TM_FORWARD` 始终正向运行，跨过午夜循环，因此从 10:00 到 08:40 时，会几乎扫过整整一天的天空，而不是向回走 80 分钟。

计算哪一边更短，并传入匹配的模式：

```ts
import { engine, SkyboxTime, TransitionMode } from '@dcl/sdk/ecs'

const DAY = 86400

// 以更短的方式移动到新的时段
function setTimeOfDay(fixedTime: number) {
  const current = SkyboxTime.getOrNull(engine.RootEntity)?.fixedTime
  // 如果尚未设置 SkyboxTime，场景就不知道当前时间，所以直接前进
  let transitionMode = TransitionMode.TM_FORWARD
  if (current !== undefined) {
    const forward = (fixedTime - current + DAY) % DAY
    // 向前超过半天意味着向后更短
    if (forward > DAY / 2) transitionMode = TransitionMode.TM_BACKWARD
  }
  SkyboxTime.createOrReplace(engine.RootEntity, { fixedTime, transitionMode })
}
```

对于你不想让玩家看到的大幅跳变，可以把它藏到全屏 UI 后面：淡入、更改时间、等待，然后淡出。

## 自定义天空、太阳和雾

有三个组件可以让你的场景在玩家站在其中时接管环境：

* `天空盒`：用你自己的纹理替换天空，或者重新着色程序化天空、云层和星星。
* `太阳`：重新着色、调暗、隐藏或固定太阳（和月亮）光的方向。
* `EnvironmentFog`：重新着色远处雾气并更改其密度。

与 `SkyboxTime`一样，这三个组件都只能添加到场景的根实体上， `engine.RootEntity`。在任何其他实体上它们都不会生效。每个字段都是可选的：你省略的任何内容都会保留其默认的一天中的时间行为。


> [!WARNING]
> **📔 注意**： `天空盒`, `太阳` 和 `EnvironmentFog` 这些组件目前还不支持移动应用或 Bevy explorer。那些客户端上的玩家会看到常规的天空、太阳和雾，因此请确保你的场景在没有这些覆盖项的情况下也能正常运行。


```ts
import { engine, Skybox, Sun, EnvironmentFog } from '@dcl/sdk/ecs'
import { Color4, Vector3 } from '@dcl/sdk/math'

function main() {
  Skybox.create(engine.RootEntity, {
    skyboxTexture: 'images/sky.png'
  })
  Sun.create(engine.RootEntity, { intensity: 0.5, direction: Vector3.create(-1, -1, 0) })
  EnvironmentFog.create(engine.RootEntity, {
    color: { keys: [{ time: 0, color: Color4.create(0.6, 0.2, 0.1, 1) }] },
    density: 0.01
  })
}
```

### 作用范围与重置

该 `天空盒`, `太阳` 和 `EnvironmentFog` 覆盖项只会在玩家位于你的场景中时生效。当玩家离开场景、某个组件被移除，或者某个字段被取消设置时，天空和照明会恢复为默认值，而再次进入场景时又会重新应用。更改会立即生效，不会有过渡。


> [!WARNING]
> **📔 注意**：在生效期间，这些覆盖项是全局性的：从你的场景内部看到的相邻地块也会以你的天空、太阳和雾来渲染。


### 自定义天空纹理

`skyboxTexture` 会用场景资源中的图像替换可见天空。像纹理的 `src` 一样把它设为图像文件的路径。该图像必须是等距柱状全景图（2:1 的纬度经度图像）：其水平中心面向场景的正 Z 轴（北方），顶部行是天顶，底部行是天底。不支持立方体十字、3x2 和六面立方体贴图布局。

```ts
import { engine, Skybox } from '@dcl/sdk/ecs'

function main() {
  Skybox.create(engine.RootEntity, {
    skyboxTexture: 'images/sky.png'
  })
}
```

图像会在水平方向平铺并进行双线性过滤。如果图像加载失败，默认天空会保持不变。

请保持图像尺寸适中：在加载场景时，整个全景图都会被下载并保留在内存中。2048x1024 像素对大多数天空已经足够；不要超过 4096x2048。请使用如 `.jpg` 这样的压缩格式用于照片风格的天空。

该纹理只会改变天空的外观：阳光仍会跟随一天中的时间，环境光仍会跟随 `skyColors` （因此你可以使它与图像匹配），并且 `太阳` 和 `EnvironmentFog` 组件仍会叠加在其上。在高质量预设下，闪亮材质上的反射也会遵循你的天空纹理。

### 天空颜色、云和星星

如果你保留程序化天空， `天空盒` 组件可以让你重新着色并调整其云层和星空。

```ts
import { engine, Skybox, ColorGradient } from '@dcl/sdk/ecs'
import { Color4 } from '@dcl/sdk/math'

// 只有一个键的渐变就是恒定颜色
function constant(color: Color4): ColorGradient {
  return { keys: [{ time: 0, color }] }
}

function main() {
  Skybox.create(engine.RootEntity, {
    skyColors: {
      zenith: constant(Color4.create(0.55, 0.25, 0.12, 1)),
      horizon: constant(Color4.create(0.95, 0.55, 0.3, 1)),
      nadir: constant(Color4.create(0.35, 0.15, 0.08, 1))
    },
    clouds: { opacity: 0.3, speed: 0.01, color: constant(Color4.create(0.9, 0.6, 0.4, 1)) },
    stars: { brightness: 4.62 }
  })
}
```

* `skyColors`：天空在其 `天顶` （正上方）、 `地平线`以及其 `天底` （地平线以下的天空，在没有地形遮挡的地方可见，例如在高海拔处或在一个拥有 [景观地形](/docs/creator/content-creator-zh/chang-jing-sdk7/xiang-mu-lei-xing/scene-metadata/#landscape-terrain) 关闭的 World 中）。这些颜色还会驱动场景的环境光：天顶颜色从上方照亮物体，地平线颜色从侧面照亮物体，天底颜色从下方照亮物体，从而使物体和头像与周围的天空相匹配。 `skyColors.rim` 是在地平线正上方绘制的一条狭窄光晕，如日落余晖；当你不设置它时，它会跟随你设置的 `地平线` 颜色（如果你设置了一个，否则它会保持默认值），因此你只在需要强调效果时才需要它（例如深色天空上的橙色日出边缘光）。
* `clouds.opacity`：从 0（无云）到 1，默认值。 `clouds.speed`：云层在天空中旋转的速度，单位为弧度/秒：默认值为 0.01（大约每 10 分钟转一圈），0 表示静态云；负值按 0 计算。 `clouds.color`：云层的色调；不设置时，即使天空被重新着色，云层也会保持默认的一天中的时间颜色。
* `stars.brightness`：星空纹理的倍率，默认值为 4.62（大于 1 的值为 HDR，0 会隐藏星星，负值按 0 计算）。星星只会在一天中的夜晚部分可见。

当设置了 `skyboxTexture` 时，纹理会替换程序化天空： `云层` 和 `星星` 不会产生影响，并且 `skyColors` 不再为天空着色，但仍然会驱动环境光。

### 太阳

该 `太阳` 组件会改变太阳和月亮的方向光。

```ts
import { engine, Sun } from '@dcl/sdk/ecs'
import { Color4, Vector3 } from '@dcl/sdk/math'

function main() {
  Sun.create(engine.RootEntity, {
    color: { keys: [{ time: 0, color: Color4.create(1, 0.65, 0.4, 1) }] },
    intensity: 0.5,
    direction: Vector3.create(-1, -1, 0)
  })
}
```

* `color`：方向光的颜色。它也会给太阳圆盘着色。
* `可见`: `true` 默认为。将其设置为 `false` 以隐藏太阳和月亮圆盘以及太阳的镜头光晕，例如当它们会透过一个 `skyboxTexture`。太阳投射的光不受影响。
* `intensity`：一天中的时间光照强度的倍率，默认值为 1。0 会关闭阳光；负值按 0 计算。
* `direction`：阳光传播的方向，使用场景坐标： `(0, -1, 0)` 会让太阳正好位于头顶， `(-1, -1, 0)` 会让它位于天空正 X（东）侧，地平线以上 45 度。设置后，太阳在任何一天中的时间都保持固定，但其颜色和强度仍会随一天中的时间变化。将其留空，或将其设置为 `(0, 0, 0)`，即可保持太阳在天空中的常规路径。

### 雾

该 `EnvironmentFog` 组件会改变远处雾气。

```ts
import { engine, EnvironmentFog } from '@dcl/sdk/ecs'
import { Color4 } from '@dcl/sdk/math'

function main() {
  EnvironmentFog.create(engine.RootEntity, {
    color: { keys: [{ time: 0, color: Color4.create(0.85, 0.5, 0.3, 1) }] },
    density: 0.01
  })
}
```

* `color`：雾的颜色。
* `density`：物体进入雾中的速度，以每米指数密度表示：给定距离处的雾为 `1 - e^(-density * distance)`，因此 `1 / density` 大约是视野有三分之二被雾化时的距离。默认值为 0.0005（约 2 公里）；0.02 会在 50 米处让大约三分之二的视野被雾化，并在 150 米时几乎全部被雾化，而 0 会使雾不可见。负值按 0 计算。

雾是否实际渲染取决于玩家选择的画质设置；场景无法将其打开或关闭。

### 一天中的颜色渐变

这些组件中的每种颜色都是一个 `ColorGradient`：一个 `键`的列表，每个键都有一个 `时间` 和一个 `Color4`相同。该 `时间` 是归一化的一天中的时间，从 0（*00:00*）到 1（*24:00*），所以 0.5 是正午。它是 `SkyboxTime` 除以 86400 后的时钟时间： `fixedTime: 21600` （上午 6 点）是 `time: 0.25`。颜色会在相邻键之间插值；在第一个键之前和最后一个键之后，会使用该键的颜色。0 到 1 之外的时间会被限制在该范围内，如果两个键共享相同的 `时间`，则列表中较靠后的那个生效。只有一个键的渐变就是恒定颜色。

```ts
import { engine, Skybox } from '@dcl/sdk/ecs'
import { Color4 } from '@dcl/sdk/math'

function main() {
  Skybox.createOrReplace(engine.RootEntity, {
    skyColors: {
      horizon: {
        keys: [
          { time: 0, color: Color4.create(0.05, 0.05, 0.2, 1) }, // 午夜
          { time: 0.25, color: Color4.create(0.95, 0.5, 0.4, 1) }, // 黎明
          { time: 0.5, color: Color4.create(0.7, 0.9, 1, 1) }, // 正午
          { time: 0.75, color: Color4.create(0.95, 0.4, 0.2, 1) }, // 黄昏
          { time: 1, color: Color4.create(0.05, 0.05, 0.2, 1) } // 再次午夜，因此不会跳变
        ]
      }
    }
  })
}
```

渐变不会在午夜处循环，所以请在 `time: 0` 和 `time: 1` 处重复相同的颜色，以便当天可以无缝循环。颜色值不受 1 的限制：大于 1 的值会产生更亮的 HDR 太阳或天空。Alpha 通道会被忽略。每个渐变最多使用 8 个键：更长的渐变会被重采样为 8 个等间距键，从而平滑它们之间的任何细节。将渐变与 `SkyboxTime` 结合使用，可将一天固定在渐变中的某个特定点。

### 完全黑暗

要让放置在场景中的灯光成为唯一的光源，就要屏蔽环境所贡献的一切：阳光及其圆盘、天空颜色（以及由此带来的环境光）、雾、云和星星。

```ts
import { engine, Skybox, Sun, EnvironmentFog } from '@dcl/sdk/ecs'
import { Color4 } from '@dcl/sdk/math'

function main() {
  const black = { keys: [{ time: 0, color: Color4.Black() }] }

  Skybox.createOrReplace(engine.RootEntity, {
    skyColors: { zenith: black, horizon: black, nadir: black },
    clouds: { opacity: 0 },
    stars: { brightness: 0 }
  })
  Sun.createOrReplace(engine.RootEntity, { intensity: 0, visible: false })
  EnvironmentFog.createOrReplace(engine.RootEntity, { color: black })
}
```


> [!WARNING]
> **📔 注意**：在低画质预设下，场景灯光会被关闭，因此对这些玩家来说，完全黑暗的场景会渲染成纯黑。使用自发光材质以保持任何关键内容可见。


参见 [灯光](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/lights/) 可向你的场景添加点光源和聚光灯。


> [!NOTE]
> **💡 提示**：有关天空盒控制的工作示例，请参见 [`2,1-skybox-sdk-scene-a`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/2,1-skybox-sdk-scene-a) 和 [`3,1-skybox-sdk-scene-b`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/3,1-skybox-sdk-scene-b) 测试场景，它们在 UI 面板中实时驱动 `SkyboxTime` 和 `TransitionMode` ，以及 [`2,0-skybox-scene-json`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/2,0-skybox-scene-json)，它读取 `fixedTime` 中设置的 `scene.json` 并在运行时通过 `getSceneInformation()`相同。该 [`2,2-skybox-control`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/2,2-skybox-control) 场景演示了这些 `天空盒`, `太阳` 和 `EnvironmentFog` 组件：天空纹理、环境预设、隐藏太阳以及完全黑暗模式。



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/skybox-control.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
