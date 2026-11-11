> 了解如何在场景中使用灯光

# 灯光

灯光是 3D 图形的基础组成部分。它们用于照亮场景并营造深度与真实感，也可用于创造不同的情绪和氛围。

默认情况下，场景由一个平行光照亮。这是一种沿特定方向照射的灯光，用于模拟太阳或月亮。见 [天空盒控制](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/skybox-control/) 了解更多信息。

在你的场景中，每个地块最多可以添加 1 盏灯。

支持两种灯光类型：

* 点光源：从某个特定点向各个方向发光的灯光。
* 聚光灯：沿特定方向发光，只覆盖一个锥形区域的灯光。


> [!WARNING]
> **📔 注**：嵌入在一个 `.glb` 或 `.gltf` 文件中的灯光会被 Decentraland 忽略。如果你的 3D 模型在导出时包含了灯光，它们不会照亮场景中的任何东西。唯一有效的光源是带有 `LightSource` 组件的实体。如果你想让灯具模型发光，请添加一个 `LightSource` 到同一个实体，或者添加到一个位置在灯泡所在处的子实体。



> [!NOTE]
> **💡 提示**：Creator Hub 资产目录中有现成的 **聚光灯** 和 **点光源** 智能物件，位于 **灯光** 类别中。每个都包含一个模型、一个配置好的 `LightSource`LightSource **聚光灯** 在 **装饰**类别下还有一种仅包含模型、不会发光的装饰项。若你需要真正的照明，请从 **灯光** 类别中选择。


## 添加灯光

要向你的场景添加灯光，你需要创建一个灯光实体，并为其添加 `LightSource` 组件。

```ts
import { engine, LightSource } from '@dcl/sdk/ecs'

const light = engine.addEntity()

Transform.create(light, {
  position: Vector3.create(10, 3, 10),
})

LightSource.create(light, {
  type: LightSource.Type.Point({})
})
```


> [!WARNING]
> **📔 注**：默认亮度的灯光在正午阳光下几乎看不见，就像现实世界一样。你可以使用 [天空盒控制](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/skybox-control/) 将天空盒强制切换到夜晚，或者通过设置 `强度` 的 `LightSource` 组件属性调高。


## 聚光灯

聚光灯是沿特定方向照射并覆盖特定锥形区域的灯光。灯光的方向由实体的 Transform 组件定义：灯光沿着实体的前向（其局部 +Z 轴）照射。锥体的开口角由 `innerAngle` 和 `outerAngle` 的 `LightSource` 组件。

下面的示例将实体绕 X 轴旋转 90 度，因此灯光会直接朝下。旋转 -90 度则会让它指向天空。

```ts
import { engine, LightSource } from '@dcl/sdk/ecs'

const light = engine.addEntity()

Transform.create(light, {
  position: Vector3.create(10, 3, 10),
  rotation: Quaternion.fromEulerDegrees(90, 0, 0),
})

LightSource.create(light, {
	type: LightSource.Type.Spot({
      innerAngle: 30,
      outerAngle: 60
    }),
	shadow: true
})
```

该 `innerAngle` 是内锥角的角度，在此范围内灯光为全亮，而 `outerAngle` 是外锥角的角度，灯光会在锥体边缘逐渐淡出。你可以通过调整这些值来创造不同效果，让灯光更聚焦或更柔和。

## 强度与颜色

所有灯光，无论是点光源还是聚光灯，都有颜色和强度。颜色由 `color` 的 `LightSource` 组件定义，而强度由 `强度` intensity

```ts
import { engine, LightSource } from '@dcl/sdk/ecs'

const light = engine.addEntity()

Transform.create(light, {
  position: Vector3.create(10, 3, 10),
})

LightSource.create(light, {
  type: LightSource.Type.Point({}),
  color: Color3.Red(),
  intensity: 10000,
})
```

颜色是一个 `Color3` 对象，如果未指定则默认为白色。你可以将其设置为任何你想要的颜色，这会对场景的氛围产生很大影响。

强度以坎德拉表示（1 米距离处的流明/平方米，或流明除以 4\*pi）。

默认强度为 16000，这相当于现实世界中一只普通灯泡的亮度，光源周围大约 10 米内都能看到。如果你需要灯光在更远处可见，或者在白天也可见，可以提高强度。

灯光可见的距离是强度值的四次方根（`intensity^0.25`).

* 强度为 625 时，灯光可见距离约为 5 米。
* 强度为 10000 时，灯光可见距离约为 10 米。
* 强度为 160000 时，灯光可见距离约为 20 米。

## 阴影

每盏灯都可以投射阴影，也可以不投射。默认情况下不会投射，但你可以通过将 `shadow` 的 `LightSource` 组件设为 `true`.

```ts
import { engine, LightSource } from '@dcl/sdk/ecs'

const light = engine.addEntity()

Transform.create(light, {
  position: Vector3.create(10, 3, 10),
  rotation: Quaternion.fromEulerDegrees(90, 0, 0),
})

LightSource.create(light, {
	type: LightSource.Type.Spot({
      innerAngle: 30,
      outerAngle: 60
    }),
	shadow: true
})
```


> [!WARNING]
> **📔 注**：阴影仅支持聚光灯。点光源不支持阴影。如果场景中有多盏灯，其中一些可能不会投射阴影，见 [灯光优化](#light-optimization) 了解更多信息。


## 开关灯光

LightSource 组件有一个 `active` 属性，可用于打开或关闭灯光。如果你想在不将灯光从场景中移除、也不将 `强度` 设为 0 并丢失原始强度值记录的情况下关闭灯光，这就很有用。

```ts
import { engine, LightSource } from '@dcl/sdk/ecs'

const light = engine.addEntity()

Transform.create(light, {
  position: Vector3.create(10, 3, 10),
  rotation: Quaternion.fromEulerDegrees(90, 0, 0),
})

LightSource.create(light, {
	type: LightSource.Type.Spot({
      innerAngle: 30,
      outerAngle: 60
    }),
	shadow: true,
	active: true
})

const lightSwitch = engine.addEntity()

Transform.create(lightSwitch, {
  position: Vector3.create(8, 1, 10),
})

MeshRenderer.setBox(lightSwitch)

MeshCollider.setBox(lightSwitch)

pointerEventsSystem.onPointerDown(
	{
		entity: lightSwitch,
		opts: {
			button: InputAction.IA_POINTER,
			hoverText: '点击',
		},
	},
	function () {
		const lightSource = LightSource.getMutable(light)
		lightSource.active = !lightSource.active
	}
)
```

## 灯光优化

灯光源会对场景性能产生相当大的影响。因此，渲染引擎会自动优化场景，通过禁用部分灯光或其阴影来提升性能，优先处理距离最远的那些。

场景中允许的活动灯光数量上限为每个地块 1 盏，除此之外还取决于用户选择的画质设置。

* 低质量：完全不显示场景灯光
* 中等质量：最多 6 盏灯（在有足够地块的场景中）
* 高质量：最多 10 盏灯（在有足够地块的场景中）
* 移动端：应用内有自己的预设，从超低画质时不显示场景灯光，到高画质时最多 8 盏灯。见 [移动端的灯光与粒子](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/kai-fa/optimize-performance/#lights-and-particles).

画质设置滑块还能进一步降低预设的上限。

如果灯光数量超过允许范围，渲染引擎会根据灯光源与玩家的距离自动禁用灯光。随着玩家移动，渲染引擎会重新启用距离玩家足够近的灯光。


> [!WARNING]
> **📔 注**：在低质量预设下，场景灯光会完全关闭。一个正确构建的 `LightSource` 则什么也不会显示，而且无论怎么调强度、范围或材质都不会改变这一点。如果一盏灯“不能工作”，请先检查画质设置。把中等质量视为你的场景支持的最低设置，而不是要求高质量。
> 
> 会发光的自发光材质并不能证明灯光正在工作。一个发光的灯具网格和它在附近表面上投射的光，是两个需要分别检查的东西。


由于有些玩家根本看不到场景灯光（桌面端低质量、移动端超低质量），请确保你的场景在没有它们的情况下仍然可用。玩家需要看到的任何内容，比如前进路线或他们可以交互的物体，都应通过自发光材质、纹理或天空盒来突出显示。将 `LightSource` 作为额外提示，而不是唯一线索。同样适用于阴影：不要让阴影成为判断物体位置的唯一方式。

在所有情况下，渲染引擎最多只会为 3 个光源渲染阴影。如果带阴影的灯光超过 3 个，渲染引擎会自动禁用其余、距离更远的灯光阴影。

除了允许的灯光最大数量之外，阴影也取决于与玩家的距离。具体距离会因灯光类型和玩家的画质设置而异，但一般规则如下：

* 距离小于 10 米：阴影渲染为柔和阴影（高质量）
* 距离在 10 到 20 米之间：阴影渲染为硬阴影（低质量）
* 距离超过 20 米：不渲染阴影

灯光本身会在更远的距离继续照亮场景：只有当玩家距离超过 160 米（10 个地块）时，它们才会被禁用。这使灯光适合大型布景，例如现场活动中的舞台灯光，因为观众大多远离光源。

另外需要注意的是，只有当玩家站在场景内部时，灯光才会被渲染。如果玩家在场景外，灯光将不会被渲染。

## 灯光范围

lightSource 组件有一个 `range` 属性，可用于设置灯光可见的最大距离。默认情况下， `range` 属性的值为 -1，这意味着灯光范围取决于灯光强度。

范围按强度值的四次方根计算（`intensity^0.25`).

* 强度为 16000 时，范围约为 11 米。
* 强度为 160000 时，范围约为 20 米。
* 强度为 1600000 时，范围约为 36 米。

默认设置可确保衰减曲线平滑且看起来自然。但如果你想限制灯光范围，可以将 `range` 属性设置为正数。

```ts
import { engine, LightSource } from '@dcl/sdk/ecs'

const light = engine.addEntity()

Transform.create(light, {
  position: Vector3.create(10, 3, 10),
})

LightSource.create(light, {
  type: LightSource.Type.Point({}),
  intensity: 16000,
  range: 20,
})
```


> [!WARNING]
> **📔 注**：将 `range` 属性设为正数会在指定距离处突然截断灯光。这在你想创建只在特定区域可见的灯光，或优化场景性能时很有用。将 `range` 设为大于当前强度下灯光实际可达到距离的值不会有任何效果。


## 灯光遮罩

你可以使用灯光遮罩来实现一些有趣的效果。与其照亮整个区域，不如应用一张纹理作为滤镜，只照亮区域中的一部分。

遮罩通常更多用于聚光灯，但点光源也可以使用。下面是一个应用了遮罩的聚光灯示例。

```ts
import { engine, LightSource } from '@dcl/sdk/ecs'

const light = engine.addEntity()

Transform.create(light, {
  position: Vector3.create(10, 3, 10),
})

LightSource.create(light, {
	type: LightSource.Type.Spot({
      innerAngle: 30,
      outerAngle: 60
    }),
	shadow: true,
	shadowMaskTexture: Material.Texture.Common({src: "assets/scene/images/lightmask1.png"})
})
```

例如，你可以应用下面这张图片，制造一种只有在纹理为白色的区域才可见灯光的有趣效果。你可以将它用于任何颜色和强度的灯光。

![](/docs/_remote/b2cd4d8e15d52341e551dffb.webp)

使用黑白图像时，灯光要么照到某个区域，要么没有照到。你也可以传入带颜色的图像，这些颜色会为灯光在各个区域着色，这可用于在表面上投射有颜色的徽标或图像。


> [!WARNING]
> **📔 注**：作为遮罩使用的图像，其像素高度和宽度必须是 2 的幂（例如：1024、512、256）。该功能不适用于尺寸不同的图像。


将遮罩应用于点光源时，纹理会以立方体的形式包裹在光源周围。如果你想避免立方体各面之间出现可见边缘，请确保纹理边缘具有连续性。

```ts
import { engine, LightSource } from '@dcl/sdk/ecs'

const light = engine.addEntity()

Transform.create(light, {
  position: Vector3.create(10, 3, 10),
})

LightSource.create(light, {
	type: LightSource.Type.Point({}),
	shadowMaskTexture: Material.Texture.Common({src: "assets/scene/images/point-light-mask1.png"})
})
```

例如，下图会将每个字母显示在立方体的不同面上（Y 在顶部，-Y 在底部，X 在右侧，-X 在左侧，Z 在前面，-Z 在后面）。

![](/docs/_remote/384393f9ee871b7e47ed3a80.webp)


> [!NOTE]
> **💡 提示**：将灯光与一个 [粒子系统](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/particle-system/) 结合使用，可获得更丰富的视觉效果。例如，将使用加法混合的火焰粒子系统与温暖的点光源搭配，就能营造出逼真的篝火效果。



> [!NOTE]
> **💡 提示**：有关此组件的可运行示例，请参见 [`0,4-dynamic-lights`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/0,4-dynamic-lights) 测试场景，其中涵盖了 `LightSource.Type.Point` 和 `LightSource.Type.Spot`、通过运行时 `LightSource.getMutable()`、阴影以及 `shadowMaskTexture`.


## 黑暗中的灯光

当天空不与它们争辉时，灯光最容易被欣赏。 `Skybox`, `Sun` 和 `EnvironmentFog` 组件可以让阳光、天空颜色（以及随之而来的环境光）、雾、云和星星全部变黑，让你的灯光成为唯一的照明来源。见 [完全黑暗](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/skybox-control/#complete-darkness).

在低质量预设下，场景灯光完全不会渲染，因此对于这些玩家来说，完全黑暗的场景会呈现为纯黑。请使用自发光材质来保持任何关键内容可见。移动端应用和 Bevy explorer 目前还不支持这些组件，因此这些客户端上的玩家仍会看到普通天空。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/lights.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
