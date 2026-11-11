> 对玩家头像施加力和冲量

# 玩家物理效果

你可以通过场景代码对玩家的化身施加物理力。这使你能够创建诸如发射台、风区、击退效果、爆炸等游戏机制。

你可以施加两种力：

* **冲量**：一次性的瞬时推动。可用于将玩家发射到空中或将其击退等突发的离散效果。
* **持续力**：只要处于激活状态，就会在每个 tick 持续施加的推动力。可用于风区、水流或重力场等持续效果。

两者都通过 `Physics` 辅助工具施加，该工具从 `@dcl/sdk/ecs`.


> [!WARNING]
> **📔 注意**：这些力只影响本地玩家的化身。其他玩家会看到其他玩家位置的变化，但在多人游戏中，力本身不会同步给其他玩家。每位玩家的物理模拟都在其自己的实例中本地运行。


## 施加冲量

使用 `Physics.applyImpulseToPlayer()` 向指定方向给予玩家一次性推动。

* `向量`：结合方向和强度——向量的长度表示冲量大小。

```ts
import { Physics } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

// 将玩家垂直向上发射
Physics.applyImpulseToPlayer(Vector3.create(0, 50, 0))
```

你也可以分别传入方向和大小。在这种情况下，方向向量会自动归一化：

* `direction`：推动方向——在缩放前会自动归一化。
* `magnitude`：冲量强度。

```ts
import { Physics } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

// 以 50 的大小将玩家向上发射
Physics.applyImpulseToPlayer(Vector3.create(0, 1, 0), 50)
```

如果你调用 `applyImpulseToPlayer()` 在同一帧内多次调用，冲量会累积——它们会相加并作为单个合并冲量施加。

### 冲量有多强

化身的质量为 1，因此冲量的大小就是化身速度的变化量，单位为米每秒。一个冲量 `50` 会沿该方向增加 50 m/s。

* 冲量所增加的速度会随时间衰减。在空中时，它以每秒 1.5 的速率衰减，因此一秒后约剩五分之一。当化身位于地面上时，衰减得更快。
* 冲量和力增加的速度上限为 50 m/s。
* 向上的冲量会先抵消化身因下落而具有的任何向下速度，因此无论玩家是走上发射台，还是从上方落到其上，发射台的效果都相同。

### 在玩家进入时触发冲量

一种常见模式是在玩家走入某个区域时触发冲量。使用触发区域来检测玩家何时进入：

```ts
import { engine, Physics, TriggerArea, triggerAreaEventsSystem, ColliderLayer, MeshRenderer, Transform } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

const launchPad = engine.addEntity()
Transform.create(launchPad, { position: Vector3.create(8, 0, 8) })
MeshRenderer.setBox(launchPad)
TriggerArea.setBox(launchPad, ColliderLayer.CL_PLAYER)

triggerAreaEventsSystem.onTriggerEnter(launchPad, (result) => {
	if (result.trigger?.entity !== engine.PlayerEntity) return
	Physics.applyImpulseToPlayer(Vector3.create(0, 50, 0))
})
```

## 施加击退冲量

使用 `Physics.applyKnockbackToPlayer()` 通过单个冲量将玩家从某个点推开。这非常适合爆炸、冲击效果和其他一次性定向冲击。方向会根据源点到玩家当前位置自动计算。

```ts
import { Physics } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

const explosionPosition = Vector3.create(8, 1, 8)

// 将玩家从爆炸中心击退
Physics.applyKnockbackToPlayer(explosionPosition, 40)
```

你可以通过 `半径`限制作用范围，并使用 `KnockbackFalloff` 选项控制大小如何随距离降低：

* `fromPosition`：击退的世界空间原点（爆炸中心、敌人位置等）。
* `magnitude`：基础冲量强度。
* `半径`：最大作用距离（默认值： `Infinity`).
* `falloff`：大小如何随距离降低（默认值： `CONSTANT`).

```ts
import { Physics, KnockbackFalloff } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

Physics.applyKnockbackToPlayer(
	Vector3.create(8, 1, 8),
	40,                          // 大小
	10,                          // 半径：超过 10 米则无效果
	KnockbackFalloff.LINEAR      // 大小在半径边缘衰减至 0
)
```

该 `KnockbackFalloff` 枚举控制力如何随距离减弱：

* `KnockbackFalloff.CONSTANT` — 半径内任意距离的大小都相同（默认）
* `KnockbackFalloff.LINEAR` — 在半径边缘平滑地线性减小至 0
* `KnockbackFalloff.INVERSE_SQUARE` — 急剧且符合物理规律地衰减

如果玩家恰好位于源位置，玩家会被直接向上推。负大小会将玩家拉向该点，而不是将其推开。

相同的 `KnockbackFalloff` 值可用于 `applyRepulsionForceToPlayer()`.

## 施加持续力

使用 `Physics.applyForceToPlayer()` 向玩家施加持续力。与冲量不同，只要该力保持激活状态，就会在每个 tick 施加。

* `source`：用于标识此力源的实体——可用于稍后更新或移除该力。
* `向量`：结合方向和强度——向量的长度表示力的大小。

源实体的位置无关紧要——它仅用作标识符。

```ts
import { engine, Physics } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

const windZoneEntity = engine.addEntity()

// 持续将玩家向右推
Physics.applyForceToPlayer(windZoneEntity, Vector3.create(10, 0, 0))
```

力向量始终位于 **世界空间**。如果你需要相对于旋转实体的方向，请使用 `Transform.localToWorldDirection()` 先将其转换——参见 [将本地方向转换为世界空间](#convert-a-local-direction-to-world-space).

与 `applyImpulseToPlayer()`一样，你也可以分别传入 `direction` 和 `magnitude` ——参见 [施加冲量](#apply-an-impulse):

```ts
Physics.applyForceToPlayer(windZoneEntity, Vector3.create(0, 1, 0), 50)
```

如果你调用 `applyForceToPlayer()` 再次使用相同的源实体调用时，它会替换该源之前的力。如果累积了多个力源，其向量会在每个 tick 相加。

### 移除持续力

要停止施加某个力，请调用 `Physics.removeForceFromPlayer()` 并传入用于创建该力的源实体引用。如果该实体当前没有施加力，此调用会被安全地忽略。

* `source`：施加力时所使用的实体。

```ts
Physics.removeForceFromPlayer(windZoneEntity)
```

### 在有限时长内施加力

使用 `Physics.applyForceToPlayerForDuration()` 在指定时间内施加力。时长单位为秒。时间到期后，力会自动移除。使用相同源实体再次调用会重置计时器。

* `source`：用于标识此力源的实体。
* `duration`：力持续的时长，单位为秒。
* `向量`：结合方向和强度——向量的长度表示力的大小。

```ts
import { engine, Physics } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

const gustEntity = engine.addEntity()

// 施加强大的向上力，持续 1.5 秒
Physics.applyForceToPlayerForDuration(gustEntity, 1.5, Vector3.create(0, 50, 0))
```

该 `direction` + `magnitude` 重载版本同样可用——参见 [施加冲量](#apply-an-impulse):

```ts
Physics.applyForceToPlayerForDuration(gustEntity, 1.5, Vector3.create(0, 1, 0), 50)
```

### 风区示例

此示例创建了一个风洞，在玩家位于其中时推动玩家，并在其离开时停止：

```ts
import { engine, Physics, TriggerArea, triggerAreaEventsSystem, ColliderLayer, MeshRenderer, MeshCollider, Transform } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

const windTunnel = engine.addEntity()
Transform.create(windTunnel, {
	position: Vector3.create(8, 1, 8),
	scale: Vector3.create(4, 3, 4),
})
MeshRenderer.setBox(windTunnel)
TriggerArea.setBox(windTunnel, ColliderLayer.CL_PLAYER)

triggerAreaEventsSystem.onTriggerEnter(windTunnel, (result) => {
	if (result.trigger?.entity !== engine.PlayerEntity) return
	Physics.applyForceToPlayer(windTunnel, Vector3.create(15, 0, 0))  // 沿 X 轴横向推动
})

triggerAreaEventsSystem.onTriggerExit(windTunnel, (result) => {
	if (result.trigger?.entity !== engine.PlayerEntity) return
	Physics.removeForceFromPlayer(windTunnel)
})
```

## 施加排斥力

使用 `Physics.applyRepulsionForceToPlayer()` 持续将玩家从空间中的固定点推开。这适用于磁性排斥场、力场屏障或让玩家保持距离的悬浮物体等效果。与 `applyKnockbackToPlayer()`不同，该力会随着玩家移动在每个 tick 重新计算。大小会根据半径和衰减随距离减弱——参见 [施加击退冲量](#apply-a-knockback-impulse) 以了解 `KnockbackFalloff` 选项。负大小会反转效果，像漩涡或重力井一样持续将玩家拉向该点。


> [!WARNING]
> **📔 注意**：排斥原点是你传入的 `fromPosition` 向量—— **不是** 该 `source` 实体的位置。该 `source` 实体仅用作标识符，以便你稍后更新或移除该力。其位置、旋转和缩放都会被完全忽略。


* `source`：用作此力标识符的实体——不会使用其位置。
* `fromPosition`：玩家被推离的世界空间点。
* `magnitude`：基础力强度。负值会吸引而不是排斥。
* `半径`：最大作用距离（默认值： `Infinity`).
* `falloff`：大小如何随距离降低（默认值： `CONSTANT`).

```ts
import { engine, Physics, Transform, timers } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

const repulsionSource = engine.addEntity()
Transform.create(repulsionSource, { position: Vector3.create(8, 1, 8) })

// 显式传入实体的位置作为排斥原点
Physics.applyRepulsionForceToPlayer(
	repulsionSource,
	Transform.get(repulsionSource).position,  // 排斥原点——显式传入，不会自动读取
	50,                                         // 大小
	10,                                         // 以米为单位的作用半径
)

// 半秒后移除力
timers.setTimeout(() => {
    Physics.removeForceFromPlayer(repulsionSource)
}, 500)
```

## 滑翔时的力

当玩家处于滑翔状态时，持续力的行为有所不同：

* 持续力——来自 `applyForceToPlayer()`, `applyForceToPlayerForDuration()`，或 `applyRepulsionForceToPlayer()` ——会 **增强 1.5 倍**，因为展开的滑翔翼会捕捉气流。风区和水流对于滑翔中的玩家而言响应更灵敏。
* 持续力的向上分量可以 **抬升** 滑翔中的玩家。滑翔翼的下落速度限制只会限制玩家下降的速度，不会抵消向上运动，因此倾斜或垂直的风流会沿力的完整方向推动玩家。

一次性冲量——来自 `applyImpulseToPlayer()` 或 `applyKnockbackToPlayer()` ——不受滑翔影响。无论滑翔翼展开还是收起，其行为都相同。

你可以组合这些行为来构建诸如将滑翔玩家向上带动的热上升气流，或在滑翔翼展开时更容易穿越的风之走廊等机制。要调整滑翔翼的下落速度或前进速度，请使用 `AvatarLocomotionSettings` 组件，参见 [移动设置](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/tou-xiang/locomotion/#locomotion-settings).

## 将本地方向转换为世界空间

使用 `Transform.localToWorldDirection()` 将方向向量从实体的本地坐标空间转换到世界空间，并考虑完整的父级层级结构。这在施加相对于旋转实体的力时很有用——例如，将玩家从旋转障碍物的特定面推开。

* `entity`：本地空间定义该方向的源实体。
* `localDirection`：实体本地坐标中的方向向量。

返回世界坐标中的方向向量。

这仅应用 **旋转** ——不考虑平移或缩放——因此可直接用于传递给力和冲量函数的方向向量。

```ts
import { Physics, Transform } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

// 沿实体本地 +Z 轴在世界空间中所指的方向推动玩家
const worldDir = Transform.localToWorldDirection(myEntity, Vector3.create(0, 0, 1))
Physics.applyImpulseToPlayer(worldDir, 20)
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/player-physics.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
