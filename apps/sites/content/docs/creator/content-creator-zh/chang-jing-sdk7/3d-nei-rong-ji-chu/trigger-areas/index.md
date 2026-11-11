# 触发区域

了解如何在场景中使用触发区域

触发区域允许你对玩家进入或离开某个区域，或者任何其他实体进入或离开某个区域的事件做出响应。这是创建交互式场景的基础工具。你可以用它来实现诸如玩家靠近时开门，或者当球进入球门时得分之类的功能。

## 使用触发区域

要使用触发区域，你需要添加一个 `TriggerArea` 组件到实体上，然后使用 `triggerAreaEventsSystem` 来响应这些事件。

```ts
import { engine, Transform, TriggerArea, triggerAreaEventsSystem } from '@dcl/sdk/ecs'

// 创建实体
const triggerEntity = engine.addEntity()

// 设置 Transform
Transform.create(triggerEntity, {
  position: Vector3.create(8, 0, 8)
  })

// 触发区域
TriggerArea.setBox(triggerEntity)

// 触发区域被激活时的事件
triggerAreaEventsSystem.onTriggerEnter(triggerEntity, function(result) {
  if (result.trigger?.entity !== engine.PlayerEntity) return;
  console.log('玩家进入了触发区域！')
})
```

默认情况下， `TriggerArea` 组件会响应以下事件： *任何* 玩家走入该区域。上面的代码添加了 `if (result.trigger?.entity !== engine.PlayerEntity) return` ，用于检查引发事件的实体是否是当前玩家，而不是其他人的头像

## 触发区域形状

触发区域可以是盒体或球体。

```ts
import { engine, Transform, TriggerArea } from '@dcl/sdk/ecs'

// 盒体
TriggerArea.setBox(triggerEntity)

// 球体
TriggerArea.setSphere(triggerEntity)
```


> [!NOTE]
> **💡 提示**: 球体是引擎最容易计算的形状，因为它只需检查球体中心的距离。如果不确定，就使用球体。


要调整触发区域的大小，你可以使用 `scale` 属性的 `变换` 组件，该组件位于持有该 `TriggerArea`.

```ts
import { engine, Transform, TriggerArea } from '@dcl/sdk/ecs'

const triggerEntity = engine.addEntity()

TriggerArea.setBox(triggerEntity)

Transform.create(triggerEntity, {
  scale: Vector3.create(4, 2, 4),
})
```

### 调试

要调试你的场景并查看触发区域覆盖的范围，你可以给带有触发区域的实体添加一个 `MeshRenderer` 组件，并将形状设置为你想要调试的形状。默认网格的尺寸将与触发区域的尺寸一致。

```ts
import { engine, Transform, TriggerArea } from '@dcl/sdk/ecs'

const triggerEntity = engine.addEntity()

TriggerArea.setBox(triggerEntity)

MeshRenderer.setBox(triggerEntity)

Transform.create(triggerEntity, {
  position: Vector3.create(8, 0, 8),
})
```

## 触发区域事件

你可以使用 `triggerAreaEventsSystem` 来响应触发区域的不同事件：

* `onTriggerEnter`: 当实体进入触发区域时触发。
* `onTriggerExit`: 当实体离开触发区域时触发。
* `onTriggerStay`: 当实体在触发区域内时，每帧都会触发。

```ts
import { engine, Transform, TriggerArea, triggerAreaEventsSystem } from '@dcl/sdk/ecs'

const triggerEntity = engine.addEntity()

TriggerArea.setBox(triggerEntity)

// 进入时
triggerAreaEventsSystem.onTriggerEnter(triggerEntity, function(result) {
  console.log('玩家进入了触发区域！')
})

// 离开时
triggerAreaEventsSystem.onTriggerExit(triggerEntity, function(result) {
  console.log('玩家离开了触发区域！')
})

// 停留时
triggerAreaEventsSystem.onTriggerStay(triggerEntity, function(result) {
  console.log('玩家在触发区域内！')
})
```

## 触发事件响应

当触发区域事件被触发时，你可以使用 `result` 参数来获取被触发的实体以及触发该事件的实体的信息。

以下属性可在 `result` 参数中使用：

* `triggeredEntity`: 被触发实体的 ID（这是拥有触发区域的实体）
* `triggeredEntityPosition`: 被触发实体的位置
* `triggeredEntityRotation`: 被触发实体的旋转
* `eventType`: 触发事件的类型（ENTER、EXIT、STAY）
* `timestamp`: 触发事件的时间戳
* `trigger`: 一个包含以下字段的对象：
  * `entity`: 触发该触发器的实体的 ID（进入触发区域的实体）
  * `layers`: 触发该触发器的实体的碰撞图层
  * `position`: 触发该触发器的实体的位置
  * `rotation`: 触发该触发器的实体的旋转
  * `scale`: 触发该触发器的实体的缩放


> [!WARNING]
> **常见混淆**: 尽管名字如此， `result.triggeredEntity` 会 **不会** 指代触发该事件的实体。这两个字段很容易混淆：
> 
> * `result.trigger?.entity`: 走入触发区域的实体。这通常是你要检查的对象，例如识别是哪位玩家或哪个物体进入了区域。
> * `result.triggeredEntity`: 拥有该 `TriggerArea` 组件的实体（也就是触发区域本身，而不是进入它的实体）。
> 
> 使用 `result.trigger?.entity` 当你需要识别进入的实体时。


```ts
import { engine, Transform, TriggerArea, triggerAreaEventsSystem } from '@dcl/sdk/ecs'

// 带有触发区域的实体
const triggerEntity = engine.addEntity()

TriggerArea.setBox(triggerEntity)

Transform.create(triggerEntity, {
  position: Vector3.create(8, 0, 8),
})

// 进入时
triggerAreaEventsSystem.onTriggerEnter(triggerEntity, function(result) {
  console.log('有实体进入了触发区域！', result.triggeredEntity)
  console.log('被触发实体位置：', result.triggeredEntityPosition)
  console.log('被触发实体旋转：', result.triggeredEntityRotation)
  console.log('事件类型：', result.eventType)
  console.log('时间戳：', result.timestamp)
  console.log('触发实体：', result.trigger.entity)
  console.log('触发层：', result.trigger.layers)
  console.log('触发位置：', result.trigger.position)
  console.log('触发旋转：', result.trigger.rotation)
  console.log('触发缩放：', result.trigger.scale)
})
```

## 检测进入区域的玩家

触发区域最常见的用途是对玩家走入或走出做出响应。两个碰撞图层可以检测头像，而使用哪一个取决于场景应该响应谁：

* `ColliderLayer.CL_MAIN_PLAYER`: 检测 **仅本地机器上的玩家**。当响应只针对该玩家时使用，例如将其传送、为其播放声音，或跟踪其任务进度。从性能角度看，这也是成本最低的选项。
* `ColliderLayer.CL_PLAYER`: 检测 **所有头像**，包括本地玩家以及场景中渲染的其他所有玩家。当任何人进入都很重要时使用，例如有人靠近就打开的门，或统计房间里有多少人。若未指定，这是默认图层。

下面的示例只对本地玩家做出响应：

```ts
import { engine, Transform, TriggerArea, ColliderLayer, triggerAreaEventsSystem } from '@dcl/sdk/ecs'

// 创建实体
const triggerEntity = engine.addEntity()

// 设置 Transform
Transform.create(triggerEntity, {
  position: Vector3.create(8, 0, 8),
})

// 仅检测本地玩家的触发区域
TriggerArea.setBox(triggerEntity, ColliderLayer.CL_MAIN_PLAYER)

// 触发区域被激活时的事件
triggerAreaEventsSystem.onTriggerEnter(triggerEntity, function (result) {
  console.log('你进入了触发区域！')
})
```

当使用 `CL_PLAYER`时，所有头像都会激活触发区域。要判断触发它的头像是否是本地玩家，请将结果中的实体与 `engine.PlayerEntity`:

```ts
import { engine, Transform, TriggerArea, triggerAreaEventsSystem } from '@dcl/sdk/ecs'

// 创建实体
const triggerEntity = engine.addEntity()

// 设置 Transform
Transform.create(triggerEntity, {
  position: Vector3.create(8, 0, 8),
})

// 触发区域，使用默认的 CL_PLAYER 图层
TriggerArea.setBox(triggerEntity)

// 触发区域被激活时的事件
triggerAreaEventsSystem.onTriggerEnter(triggerEntity, function (result) {
  if (result.trigger?.entity === engine.PlayerEntity) {
    console.log('你进入了触发区域！')
  } else {
    console.log('其他玩家进入了触发区域！')
  }
})
```

在大多数情况下，与其尝试检测所有玩家，不如只检测当前玩家（通过 `CL_MAIN_PLAYER`），然后在所有玩家之间同步这个触发器的效果。例如，滑动门只为你的头像打开，但门的开启状态随后会共享给所有其他玩家。


> [!NOTE]
> **💡 提示**: 仅包含头像图层（`CL_PLAYER`, `CL_MAIN_PLAYER`，或两者都有）的触发区域都经过优化：引擎会将它们放在仅限头像的物理图层上，因此与头像以外任何对象的重叠都会在到达场景代码之前被物理引擎丢弃。掩码恰好等于 `CL_MAIN_PLAYER` 则会得到进一步优化：它会在到达处理器之前，直接短路与非本地玩家碰撞体的任何重叠，因此只检测本地玩家几乎没有性能开销。


## 触发区域图层

使用 `TriggerArea` 组件的可选第二个参数来设置会激活触发区域的图层。默认情况下，触发区域仅由图层 `ColliderLayer.CL_PLAYER`激活，见 [检测进入区域的玩家](#detect-players-entering-an-area).

你可以通过将其作为 `TriggerArea` 组件的实体时，组件不会被激活。

```ts
import { engine, Transform, TriggerArea, MeshCollider, ColliderLayer, triggerAreaEventsSystem } from '@dcl/sdk/ecs'

// 触发区域
const triggerEntity = engine.addEntity()

TriggerArea.setBox(triggerEntity, ColliderLayer.CL_CUSTOM1)

Transform.create(triggerEntity, {
  position: Vector3.create(8, 0, 8),
})

// 将激活触发区域的实体
const movingEntity = engine.addEntity()

Transform.create(movingEntity, {
  position: Vector3.create(8, 0, 8),
})

MeshCollider.setBox(movingEntity, ColliderLayer.CL_CUSTOM1)
```

可用值与 `MeshCollider` 组件遍历所有玩家。参见 [碰撞图层](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/colliders/#collision-layers) 以了解更多详情。

* `ColliderLayer.CL_PLAYER`: 任何头像（本地 + 远程）
* `ColliderLayer.CL_MAIN_PLAYER`: 仅本地玩家
* `ColliderLayer.CL_PHYSICS`
* `ColliderLayer.CL_POINTER`
* `ColliderLayer.CL_CUSTOM1` 一直到 `CL_CUSTOM8`
* `ColliderLayer.CL_NONE`


> [!NOTE]
> **💡 提示**: 图层 `CL_CUSTOM1` 一直到 `CL_CUSTOM8` 本身没有任何特殊行为，你可以根据场景需要自由使用它们。


你也可以设置一个触发区域同时检测多个图层。

```ts
import { engine, Transform, TriggerArea, ColliderLayer, triggerAreaEventsSystem } from '@dcl/sdk/ecs'

const triggerEntity = engine.addEntity()

TriggerArea.setBox(triggerEntity, ColliderLayer.CL_CUSTOM1 | ColliderLayer.CL_CUSTOM2)

Transform.create(triggerEntity, {
  position: Vector3.create(8, 0, 8),
})
```

当任何具有以下图层的实体 `CL_CUSTOM1` 或 `CL_CUSTOM2` 进入触发区域时，这将激活该触发区域。


> [!NOTE]
> **💡 提示**: 如需一个可运行的示例来完整演示 `TriggerArea` 端到端，请查看 [`75,-9-trigger-areas`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/75,-9-trigger-areas) 测试场景，其中涵盖 ENTER/EXIT 的线协议以及 SDK 的逐帧 `onTriggerStay` 合成、旋转区域、位于自定义碰撞层上的移动触发者，以及附着在下落实体上的区域。如需查看碰撞层本身的行为，请参阅 [`5,5-collider-layers`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/5,5-collider-layers).



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/trigger-areas.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
