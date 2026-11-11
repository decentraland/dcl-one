# 粒子系统

向场景添加火焰、雨、火花和魔法光环等粒子效果

粒子系统可以通过发射并动画化大量小精灵来创建动态视觉效果。可用它们来制作火焰、烟雾、雨、雪、火花、魔法光环、爆炸以及许多其他用静态网格难以实现的效果。

## 添加粒子系统

要向场景中添加粒子系统，请创建一个实体并附加 `ParticleSystem` 组件。

```ts
import { engine, Transform, ParticleSystem } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

const emitter = engine.addEntity()

Transform.create(emitter, {
	position: Vector3.create(8, 1, 8),
})

ParticleSystem.create(emitter, {})
```

如果不设置任何属性，该组件会使用全部默认值：一个点状发射器，每秒发射 10 个粒子，粒子生命周期为 5 秒。

## 发射器形状

发射器形状决定粒子在何处生成。可通过 `ParticleSystem.Shape` 辅助函数使用四种形状。

### 点

粒子从单个点（实体的位置）生成。当未指定形状时，这就是默认值。

```ts
ParticleSystem.create(emitter, {
	shape: ParticleSystem.Shape.Point(),
})
```

### 球体

粒子从球体表面或内部生成。

```ts
ParticleSystem.create(emitter, {
	shape: ParticleSystem.Shape.Sphere({ radius: 2 }),
})
```

### 锥体

粒子从锥体底部生成，并沿锥体方向向外运动。 `angle` 是以度为单位的半角； `radius` 是底部半径，单位为米。要调整锥体方向，请设置实体的 `Transform`.

```ts
ParticleSystem.create(emitter, {
	shape: ParticleSystem.Shape.Cone({ angle: 25, radius: 1 }),
})
```

### 盒体

粒子从盒状体积内部的任意位置生成。

```ts
ParticleSystem.create(emitter, {
	shape: ParticleSystem.Shape.Box({ size: Vector3.create(3, 1, 3) }),
})
```

## 发射属性

控制创建多少粒子以及它们存活多久。

| 属性             | 默认值    | 说明             |
| -------------- | ------ | -------------- |
| `rate`         | `10`   | 每秒发射的粒子数（连续）。  |
| `maxParticles` | `1000` | 同时存活粒子的硬上限。    |
| `lifetime`     | `5`    | 每个粒子的存活时间（秒）。  |
| `active`       | `true` | 系统是否正在主动发射新粒子。 |

```ts
ParticleSystem.create(emitter, {
	rate: 50,
	maxParticles: 500,
	lifetime: 3,
})
```

## 运动

### 重力

该 `gravity` 属性是应用于场景重力（-9.81 m/s²）的乘数。将其设置为 `0` 可让粒子悬浮在原地，设置负值会将它们向上推，设置正值会使它们向下加速。

```ts
ParticleSystem.create(emitter, {
	gravity: -0.5, // 粒子缓慢向上漂浮
})
```

### 初始速度

`initialVelocitySpeed` 是一个 `FloatRange` ，用于为每个粒子的发射随机速度范围设置最小值和最大值（单位为 m/s）。两个值的默认值均为 `1`.

```ts
ParticleSystem.create(emitter, {
	initialVelocitySpeed: { start: 2, end: 8 },
})
```

### 附加力

除重力之外，每帧都会施加到所有粒子上的恒定力向量。适用于风或磁力效果。

```ts
ParticleSystem.create(emitter, {
	additionalForce: Vector3.create(0.5, 0, 0), // 恒定的侧向推动
})
```

### 速度限制

使用 `limitVelocity` 用于限制粒子的最大速度，并可选择在每帧削减其多余速度。

```ts
ParticleSystem.create(emitter, {
	limitVelocity: {
		speed: 5, // 允许的最大速度（m/s）
		dampen: 0.8, // 每帧移除的多余速度比例
	},
})
```

## 视觉属性

### 颜色

`initialColor` 设置粒子出生时的颜色，会在给定范围内随机选取。 `colorOverTime` 使粒子的颜色在生命周期内逐渐从 `开始` 到 `结束` 值过渡。两者都接受一个 `ColorRange` 并使用一个 `开始` 和 `结束` 值。

```ts
import { Color4 } from '@dcl/sdk/math'

ParticleSystem.create(emitter, {
	initialColor: {
		start: Color4.create(1, 0.5, 0, 1), // 橙色
		end: Color4.create(1, 1, 0, 1), // 黄色
	},
	colorOverTime: {
		start: Color4.create(0.5, 0, 0, 0.5), // 暗红色，半透明
		end: Color4.create(0, 0, 0, 0), // 完全透明（淡出）
	},
})
```

请注意， `Color4` 中第 4 个值是 *alpha*透明度（alpha）。如果将最终颜色设置为 alpha 为 0 的值，粒子会逐渐淡出并变得不可见，这通常是个不错的效果。

### 大小

`initialSize` 和 `sizeOverTime` 都是 `FloatRange` 权重值播放。 `initialSize` 控制粒子出生时的缩放，取值会在给定范围内随机选择。 `sizeOverTime` 使粒子的缩放在生命周期内逐渐从 `开始` 到 `结束` 值变化为 `1` 等于原始纹理大小。

```ts
ParticleSystem.create(emitter, {
	initialSize: { start: 0.1, end: 0.3 },
	sizeOverTime: { start: 0.5, end: 1.0 }, // 在生命周期内增大
})
```


> [!WARNING]
> **📔 注意**： `缩放` 实体的 `Transform` 不会影响粒子的缩放。


### 纹理

默认情况下，粒子会渲染为白色方块。提供一个纹理即可使用自定义图像。

```ts
ParticleSystem.create(emitter, {
	texture: { src: 'assets/scene/Images/spark.png' },
})
```

### 混合模式

控制粒子颜色与其背后场景的合成方式。

| 值                                      | 说明                        |
| -------------------------------------- | ------------------------- |
| `ParticleSystemBlendMode.PSB_ALPHA`    | 标准透明度（默认）。                |
| `ParticleSystemBlendMode.PSB_ADD`      | 加色混合——粒子会使场景变亮。适合火焰和发光效果。 |
| `ParticleSystemBlendMode.PSB_MULTIPLY` | 将粒子颜色与其背后的场景相乘。           |

```ts
import { ParticleSystemBlendMode } from '@dcl/sdk/ecs'

ParticleSystem.create(emitter, {
	texture: { src: 'assets/scene/textures/ember.png' },
	blendMode: ParticleSystemBlendMode.PSB_ADD,
})
```

### 广告牌

当 `billboard` 是 `true` （默认）时，无论发射器朝向如何，每个粒子都会始终面向相机。将其设置为 `false` false，可用于应在 3D 空间中翻滚的粒子。

```ts
ParticleSystem.create(emitter, {
	billboard: false,
})
```

## 旋转

### 初始旋转和随时间变化的旋转

`initialRotation` 是粒子生成时的朝向。 `rotationOverTime` 是每秒按各轴施加的角速度。两者都接受一个 `Quaternion`.

```ts
import { Quaternion } from '@dcl/sdk/math'

ParticleSystem.create(emitter, {
	initialRotation: Quaternion.fromEulerDegrees(0, 0, 45),
	rotationOverTime: Quaternion.fromEulerDegrees(0, 0, 90), // 绕 Z 轴以 90°/秒旋转
})
```


> [!WARNING]
> **📔 注意**：如果 `billboard` 被设为 `true`，则粒子只会绕一个轴旋转，并始终保持面向相机的朝向。


### 面向运动方向

当 `faceTravelDirection` 是 `true`，每个粒子都会自动旋转，指向其运动方向，就像小行星或叶子在空中飘落一样。

```ts
ParticleSystem.create(emitter, {
	faceTravelDirection: true,
	billboard: false,
})
```


> [!WARNING]
> **📔 注意**：如果 `faceTravelDirection` 如果为 true， `billboard` 的值将被忽略。


## 精灵表动画

你可以将纹理视为一张动画帧网格来为粒子制作动画。指定表中的列数、行数以及播放速度。

```ts
ParticleSystem.create(emitter, {
	texture: { src: 'assets/scene/textures/flame-sheet.png' },
	spriteSheet: {
		tilesX: 4, // 4 列
		tilesY: 3, // 3 行（共 12 帧）
		framesPerSecond: 12,
	},
})
```

## 播放控制

粒子系统可处于由 `playbackState`.

| 值                                        | 说明             |
| ---------------------------------------- | -------------- |
| `ParticleSystemPlaybackState.PS_PLAYING` | 主动发射中（默认）。     |
| `ParticleSystemPlaybackState.PS_PAUSED`  | 冻结所有当前粒子并停止发射。 |
| `ParticleSystemPlaybackState.PS_STOPPED` | 停止发射并移除所有现有粒子。 |

```ts
import { ParticleSystemPlaybackState } from '@dcl/sdk/ecs'

// 玩家走远时暂停
ParticleSystem.getMutable(emitter).playbackState =
	ParticleSystemPlaybackState.PS_PAUSED

// 恢复
ParticleSystem.getMutable(emitter).playbackState =
	ParticleSystemPlaybackState.PS_PLAYING
```

### 循环与预热

默认情况下，系统会无限循环。将 `循环` 设置为 `false` 用于在所有粒子死亡后自动停止的一次性效果。

将 `prewarm` 设置为 `true` （需要 `loop: true`）以模拟系统仿佛从一开始就已运行，因此玩家第一次看到它时，粒子已经填满场景。

```ts
ParticleSystem.create(emitter, {
	loop: false, // 仅播放一次然后停止
	prewarm: false,
})
```

## 爆发发射

爆发让你在特定时刻发射大量粒子，而不是以恒定速率发射。它们适合爆炸、烟花或其他一次性事件。

单个粒子系统可以经历包含多个爆发的循环，即使间隔或概率不同，也会显得更自然。

```ts
ParticleSystem.create(emitter, {
	rate: 0, // 禁用连续发射
	bursts: {
		values: [
			{
				time: 0, // 播放开始后的时间（秒）
				count: 200, // 每次爆发发射的粒子数
				cycles: 1, // 重复次数（0 = 无限）
				interval: 0.2, // 重复循环之间的秒数
				probability: 1, // 每个循环中该爆发触发的概率（0–1）
			},
		],
	},
})
```

对于带有错时爆发的循环烟花效果：

```ts
ParticleSystem.create(emitter, {
	loop: true,
	rate: 0,
	lifetime: 2,
	bursts: {
		values: [
			{ time: 0.0, count: 80, cycles: 0, interval: 3 },
			{ time: 0.7, count: 100, cycles: 0, interval: 3 },
			{ time: 1.4, count: 60, cycles: 0, interval: 3 },
		],
	},
})
```

## 模拟空间

控制粒子是相对于发射器移动（`PSS_LOCAL`，默认）还是在生成后保持在世界坐标中固定不动（`PSS_WORLD`).

使用 `PSS_WORLD` 适用于会移动并应在其后留下轨迹的发射器（例如小行星或火箭）。

```ts
import { PBParticleSystem_SimulationSpace } from '@dcl/sdk/ecs'

ParticleSystem.create(emitter, {
	simulationSpace: PBParticleSystem_SimulationSpace.PSS_WORLD,
})
```

## 性能

引擎会对每个场景施加粒子预算；如果总粒子数可能超出上限，它会自动降低场景中所有活动粒子系统的发射率。请相应规划你的场景：

* 优先选择更少但视觉冲击更强的系统，而不是许多效果不明显的系统。
* 使用 `maxParticles` 用于限制单个系统的上限。
* 使用 `active` 或 `playbackState` 用于禁用离屏或超出范围的系统。
* 较短 `lifetime` 的值可让存活粒子数量低于高发射率 `rate` 本身所暗示的数量。

引擎会将任意时刻渲染的最大粒子数上限设为 1000。如果你发射的粒子超过这个数量，可能无法看到全部粒子。

另外要记住，粒子只有在玩家站在你的场景内部时才能看到。站在场景外部观看场景的玩家，直到进入场景之前都看不到任何粒子。

## 粒子实验室

进入世界 [ParticleLab.dcl.eth](decentraland://?realm=particlelab.dcl.eth\&dclenv=org) 来试验不同的粒子系统。当你靠近某个粒子系统时，界面会显示所有可用字段，你可以直接实时调整它们，而无需重新加载场景。满意后，点击 **复制** 按钮即可将该粒子系统的代码复制到剪贴板。


> [!NOTE]
> **💡 提示**：如需一个可运行示例，演示每个 `ParticleSystem` 字段，请查看 [`0,7-particle-system`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/0,7-particle-system) 测试场景。它涵盖了四种发射器形状以及 `PSB_ADD`/`PSB_ALPHA`/`PSB_MULTIPLY` 混合模式，以及 `PS_PLAYING`/`PS_PAUSED`/`PS_STOPPED` 播放状态、精灵表、连续 `rate` 与一次性和循环 `爆发`，和 `PSS_LOCAL` 与 `PSS_WORLD` 在移动的发射器上。



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/particle-system.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
