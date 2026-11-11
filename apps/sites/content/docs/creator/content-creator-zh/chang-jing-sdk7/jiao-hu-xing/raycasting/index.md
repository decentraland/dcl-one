> 使用射线投射在空间中追踪一条线，并查询与场景中实体的碰撞。

# 射线投射

射线投射是游戏开发中的一项基础工具。使用射线投射，你可以在空间中追踪一条假想的线，并查询是否有任何实体与这条线相交。这对于计算视线、子弹轨迹、寻路算法以及许多其他应用都很有用。

当玩家按下指针按钮，或主按钮或次按钮时，会从玩家位置沿他们注视的方向发射一条射线，参见 [按钮事件](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/click-events/) 了解更多细节。本文档介绍如何从任意给定的位置和方向追踪一条不可见的射线，而不依赖玩家操作，你可以将其用于许多其他场景。

请注意，射线投射只能击中带有碰撞体的对象。因此，如果你想检测射线是否击中某个 3D 模型，可以：

* 模型必须包含 [碰撞网格](/docs/creator/content-creator-zh/3d-jian-mo-yu-dong-hua/colliders/).
* 该 `GLTFContainer` 必须配置为使用 [带有碰撞遮罩的可见几何体](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/colliders/#colliders-on-3d-models).
* 添加一个 [MeshCollider 组件](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/colliders/).

此外，为自定义 [碰撞层](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/colliders/#collision-layers) 碰撞层分配给 3D 模型，这样射线只需计算与相关实体的碰撞，而不是与所有带有碰撞体的对象计算碰撞。

## 创建一条射线

所有射线都有一个起点和一个方向。起点基于某个实体的位置，取自该实体的 Transform 组件中的值。射线的方向可以用 4 种不同方式定义：

* **局部**：相对于实体正前方方向的方向，也会受到任何父实体变换的影响。这对于检测车辆前方的障碍物并保持其朝向很有用。
* **global**：忽略实体的旋转，并让其朝向某个方向，就像实体旋转为 0 一样。这例如适用于始终朝下。
* **全局目标**：在实体的位置与场景中的一个目标全局位置之间追踪一条线。它会忽略实体的旋转。例如，这对于制作塔防游戏很有用，每座塔的炮塔都可以指向空间中的一个精确坐标。
* **目标实体**：在实体的位置与第二个目标实体的位置之间追踪一条线。它会忽略任一实体的旋转。

以下代码创建一个带有本地方向的射线投射：

```ts
const myEntity = engine.addEntity()
Transform.create(myEntity, {
  position: Vector3.create(4, 1, 4),
})

raycastSystem.registerLocalDirectionRaycast(
  {
    entity: myEntity,
    opts: { direction: Vector3.Forward() },
  },
  function (raycastResult) {
    // 回调函数
  }
)
```

使用以下函数以不同方式提供方向来创建射线投射：

* `raycastSystem.registerLocalDirectionRaycast()`：创建一个带有 **局部** 方向的射线投射。该 `方向` 字段期望一个 `Vector3` ，它描述一个相对于实体及其旋转的向量（例如 `Vector3.Forward()` 最终会使用实体变换中的前向向量）
* `raycastSystem.registerGlobalDirectionRaycast()`：创建一个带有 **global** 方向的射线投射。该 `方向` 字段期望一个 `Vector3` ，它描述全局方向。
* `raycastSystem.registerGlobalTargetRaycast()`：创建一个方向由某个 **全局目标** 位置。该 `目标` 字段期望一个 `Vector3` ，它描述场景中的一个全局位置。
* `raycastSystem.registerTargetEntityRaycast()`：创建一个方向指向某个 **目标实体** 位置。该 `targetEntity` 字段期望一个实体引用，该实体的位置将用作射线的目标。

使用以上任一方法创建射线时，可用以下可选字段：

* `maxDistance`: *数字* 用于设置这条射线的追踪长度。如果未设置，默认值为 16 米。
* `queryType`: *RaycastQueryType* 枚举值，用于定义射线是返回所有命中的实体还是只返回第一个。可用选项如下：
  * `RaycastQueryType.RQT_HIT_FIRST`: *（默认）* 只返回从起点开始遇到的第一个命中实体。
  * `RaycastQueryType.RQT_QUERY_ALL`：返回所有命中的实体，从起点一直到射线的最大距离。
* `originOffset`：不要从实体的原点位置开始射线投射，而是添加一个偏移量，从相对位置开始查询。比如你可以使用一个小偏移量来防止射线与实体自身的碰撞体发生碰撞。如果未设置，默认值为 `Vector3.Zero()`.
* `collisionMask`：仅检测与特定碰撞层的碰撞。将其与自定义碰撞层一起使用，或者仅检测物理层或指针事件层。参见 [碰撞层](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/colliders/#collision-layers)。如果未设置，默认使用的层为 `ColliderLayer.CL_PHYSICS`.
* `continuous`：如果为 true，则会在每一帧持续运行射线投射查询。如果为 false，则射线只会用于当前帧。如果未设置，默认值为 false。
* 当使用本地方向或全局方向设置方向时， `方向` 字段默认值为 `Vector3.Forward()`.
* 当使用全局目标设置方向时， `目标` 字段默认值为 `Vector3.Zero()`.
* 当使用实体目标设置方向时， `targetEntity` 字段默认指向场景的根实体，位于 `Vector3.Zero()`.


> [!WARNING]
> **📔 注意**： `continuous` 属性应谨慎使用，因为每一帧运行射线投射查询的性能开销可能很大。在可能的情况下，使用一个 system（或 Utils 库中的 `interval` 函数）以更稀疏的固定间隔运行射线投射查询，参见 [循环射线投射](#recurrent-raycasting).


以下是使用这四种方法来确定射线方向的示例：

```ts
// 本地方向射线投射
raycastSystem.registerLocalDirectionRaycast(
  {
    entity: myEntity,
    opts: {
      queryType: RaycastQueryType.RQT_QUERY_ALL,
      direction: Vector3.Forward(),
      maxDistance: 30,
    },
  },
  function (raycastResult) {
    console.log(raycastResult.hits)
  }
)
// 全局方向射线投射
raycastSystem.registerGlobalDirectionRaycast(
  {
    entity: myEntity,
    opts: {
      queryType: RaycastQueryType.RQT_QUERY_ALL,
      direction: Vector3.Forward(),
      maxDistance: 30,
    },
  },
  function (raycastResult) {
    console.log(raycastResult.hits)
  }
)
// 全局目标位置射线投射
raycastSystem.registerGlobalTargetRaycast(
  {
    entity: myEntity,
    opts: {
      queryType: RaycastQueryType.RQT_QUERY_ALL,
      target: Vector3.Zero(),
    },
  },
  (raycastResult) => {
    console.log(raycastResult.hits)
  }
)
// 目标实体射线投射
const targetEntity = engine.addEntity()
Transform.create(targetEntity, { position: Vector3.create(8, 1, 10) })

raycastSystem.registerTargetEntityRaycast(
  {
    entity: myEntity,
    opts: {
      queryType: RaycastQueryType.RQT_QUERY_ALL,
      targetEntity: targetEntity,
    },
  },
  (raycastResult) => {
    console.log(raycastResult.hits)
  }
)
```


> [!WARNING]
> **📔 注意**: `raycastSystem`, `RaycastQueryType` 和 `ColliderLayer` 必须通过
> 
> > `import { raycastSystem, RaycastQueryType, ColliderLayer } from "@dcl/sdk/ecs"`
> 
> 参见 [导入](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/coding-scenes/#imports) 以了解如何轻松处理这些内容。


## 射线投射结果

处理射线投射的回调函数会接收一个对象，其中包含关于射线本身以及任何被击中的实体的数据。

* `globalOrigin`：射线起始的位置，相对于场景。
* `方向`：射线指向的全局方向，作为一个 `Vector3`.
* `hits`：一个数组，其中每个被击中的实体对应一个对象。如果没有命中的实体，则此数组为空。如果射线投射使用了 `RaycastQueryType.RQT_HIT_FIRST`，那么该数组只会包含一个对象。

该 `hits` 数组中的每个对象都包含：

* `entityId`：被射线击中的实体的 ID 编号。
* `meshName`: *字符串* ，其中包含被击中的 3D 模型中具体网格的内部名称。当一个 3D 模型由多个网格组成时，这很有用。
* `位置`: *Vector3* 射线与命中实体相交的位置（相对于场景）
* `length`：射线从起点到与实体发生命中的位置之间的长度。
* `normalHit`: *Vector3* 用于表示世界空间中命中表面的法线。
* `globalOrigin`: *Vector3* 射线起始位置（相对于场景）
* `方向`：射线指向的全局方向，作为一个 `Vector3`.

以下示例会遍历被击中的实体：

```ts
const myEntity = engine.addEntity()
Transform.create(myEntity, {
  position: Vector3.create(4, 1, 4),
})

raycastSystem.registerLocalDirectionRaycast(
  {
    entity: myEntity,
    opts: {
      queryType: RaycastQueryType.RQT_QUERY_ALL,
      direction: Vector3.Forward(),
      maxDistance: 30,
    },
  },
  function (raycastResult) {
    if (raycastResult.hits.length > 0) {
      for (const hit of raycastResult.hits) {
        if (hit.entityId) {
          console.log('击中了实体 ', hit.entityId)
        }
      }
    } else {
      console.log('未击中任何实体')
    }
  }
)
```


> [!WARNING]
> **📔 注意**：你可以从击中另一个场景中的实体获得射线投射结果。


## 处理被击中的实体

当你获得一个击中实体的射线投射结果时，你可以使用 `entityId` 与实体及其组件交互。实体不过是 [只是一个数字](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/entities-components/#overview)，因此 `entityId` 该值本身可以被解释为一个 `Entity` 类型。

```ts
const hitEntity = hit.entityId as Entity
const transform = Transform.get(hitEntity)
console.log(transform.position)
```

## 碰撞层

最好只检测与相关实体之间的碰撞，这样可以提升场景性能。该 `collisionMask` 字段允许你只列出特定的碰撞层——物理层（场景墙壁和地板）、指针层（指针事件）、玩家层（头像），或你可以自由分配的 8 个自定义层。参见 [碰撞层](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/colliders/#collision-layers).

默认情况下， `collisionMask` 字段设置为 `ColliderLayer.CL_PHYSICS`。你可以更改此值以列出其他层，或者使用 `|` 分隔符将多个层组合起来。

```ts
raycastSystem.registerLocalDirectionRaycast(
  {
    entity: myEntity,
    opts: {
      queryType: RaycastQueryType.RQT_QUERY_ALL,
      direction: Vector3.Forward(),
      maxDistance: 30,
      collisionMask:
        ColliderLayer.CL_CUSTOM1 |
        ColliderLayer.CL_CUSTOM3 |
        ColliderLayer.CL_POINTER,
    },
  },
  (raycastResult) => {
    log(raycastResult.hits)
  }
)
```

## 循环射线投射

当使用……的函数时 `raycastSystem`，默认行为是创建一条单次查询碰撞的射线。或者，你可以将 `continuous` 字段设置为 *true* 以便在游戏循环的每个 tick 中运行一次查询和回调函数。

以下示例将从此时起持续运行射线投射查询

```ts
raycastSystem.registerLocalDirectionRaycast(
  {
    entity: myEntity,
    opts: {
      queryType: RaycastQueryType.RQT_QUERY_ALL,
      direction: Vector3.Forward(),
      maxDistance: 30,
      continuous: true,
    },
  },
  function (raycastResult) {
    log(raycastResult.hits)
  }
)
```


> [!WARNING]
> **📔 注意**： `continuous` 属性应谨慎使用，因为每一帧运行射线投射查询的性能开销可能很大。


当不再需要时，请移除任何循环射线投射。为此，你必须使用 `raycastSystem.removeRaycasterEntity`.

```ts
raycastSystem.removeRaycasterEntity(myEntity)
```

在可能的情况下，使用一个 system（或 `interval` Utils 库中的函数）以更规律且更稀疏的间隔运行射线投射查询，比如每秒一次，或每五分之一秒一次。

```typescript
// 自定义组件
const CubeOscilator = engine.defineComponent('CubeOscilator', {
  t: Schemas.Float,
})

const TimerComponent = engine.defineComponent('TimerComponent', {
  t: Schemas.Float,
})

const RAY_INTERVAL = 0.1

// 检查射线
engine.addSystem((dt) => {
  for (const [entity] of engine.getEntitiesWith(TimerComponent)) {
    const timer = TimerComponent.getMutable(entity)
    timer.t += dt

    if (timer.t > RAY_INTERVAL) {
      timer.t = 0
      raycastSystem.registerGlobalDirectionRaycast(
        {
          entity: myEntity,
          opts: {
            queryType: RaycastQueryType.RQT_HIT_FIRST,
            direction: Vector3.Forward(),
            maxDistance: 16,
          },
        },
        function (raycastResult) {
          log(raycastResult.hits)
        }
      )
    }
  }
})

TimerComponent.create(engine.addEntity())

// 振荡立方体系统
engine.addSystem((dt) => {
  for (const [entity, cube] of engine.getEntitiesWith(
    CubeOscilator,
    Transform
  )) {
    CubeOscilator.getMutable(entity).t += dt
    Transform.getMutable(entity).position.y = 2 + Math.cos(cube.t)
  }
})

// 创建立方体
const cubeEntity = engine.addEntity()
Transform.create(cubeEntity, { position: { x: 8, y: 1, z: 8 } })
CubeOscilator.create(cubeEntity)
MeshRenderer.setBox(cubeEntity)
MeshCollider.setBox(cubeEntity)
```

上面的示例每 0.1 秒运行一次循环射线投射。它使用一个计时器组件和一个系统的 `dt` 属性来均匀计时。它还包含一个上下振荡的立方体，由另一个系统控制，使其进出射线的路径。


> [!NOTE]
> **💡 提示**：使用场景编辑器中的 `interval` 中的函数 [SDK Utils 库](https://github.com/decentraland/sdk7-utils) 以便用更简单的方式按固定间隔运行一个函数。


## 通过系统进行射线投射

另一种执行循环射线投射的方法是在系统的循环函数中执行它们。这样你可以更灵活地控制它们何时以及如何工作。你可以不用注册回调函数，而是通过 `raycastSystem.registerRaycast` 然后在系统函数内检查此操作返回的数据。

请注意，由于射线投射是在系统中执行的，因此结果要到下一个 tick 才会可用，这需要系统运行两次。一次用于为下一帧注册射线投射，下一帧再处理其结果。

```ts
engine.addSystem((deltaTime) => {
		const result = raycastSystem.registerRaycast(
			myEntity,
			raycastSystem.localDirectionOptions({
				collisionMask: ColliderLayer.CL_CUSTOM1 | ColliderLayer.CL_CUSTOM3 | ColliderLayer.CL_POINTER,
				originOffset: Vector3.create(0, 0.4, 0),
				maxDistance: 16,
				queryType: RaycastQueryType.RQT_HIT_FIRST,
				direction: Vector3.Forward(),
				continuous: true // 不要过度使用 'continuous' 属性，因为射线投射的性能开销很大
			})
		)
		if (result) {
			// 执行一些操作
		}
	})
```

## 与玩家碰撞

你可以通过在遮罩中包含任一头像碰撞层，直接使用射线投射检测头像：

* `ColliderLayer.CL_PLAYER`：匹配任意头像——本地玩家以及场景中渲染的任何其他玩家。
* `ColliderLayer.CL_MAIN_PLAYER`：只匹配本地（主）玩家。

这两个层可以组合使用，也可以单独使用。射线投射的默认 `collisionMask` 值为 `CL_PHYSICS`，它不会击中头像——你必须显式启用。

```ts
// 只击中本地玩家（忽略其他头像）
raycastSystem.registerLocalDirectionRaycast(
  {
    entity: myEntity,
    opts: {
      direction: Vector3.Forward(),
      collisionMask: ColliderLayer.CL_MAIN_PLAYER,
    },
  },
  (raycastResult) => {
    if (raycastResult.hits.length > 0) {
      console.log('击中了本地玩家')
    }
  }
)

// 击中任意头像（本地 + 远程）
raycastSystem.registerLocalDirectionRaycast(
  {
    entity: myEntity,
    opts: {
      direction: Vector3.Forward(),
      collisionMask: ColliderLayer.CL_PLAYER,
    },
  },
  (raycastResult) => {
    // 对于远程头像的命中，raycastResult.hits[i].entityId 为 0（没有场景本地实体 ID）
    console.log(raycastResult.hits)
  }
)
```

当射线击中本地玩家时， `hit.entityId` 是 `engine.PlayerEntity`。对远程头像的命中不包含场景本地实体 ID（该字段为 `0`）因为远程玩家并不是你场景世界中的一个实体——但命中仍会附带其 `位置`, `length`, `normalHit`以及其他几何数据。


> [!NOTE]
> **💡 提示**：若要仅检测 **其他** 玩家（排除本地玩家），请使用 `CL_PLAYER` 并在回调中筛选 `hit.entityId !== engine.PlayerEntity` 。


## 从玩家发出的射线投射

要从玩家的位置沿摄像机朝向的方向追踪射线，你可以使用摄像机或头像来追踪一条射线 [保留实体](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/entities-components/#reserved-entities).


> [!NOTE]
> **💡 提示**：在大多数情况下，你可能更适合使用 [指针事件](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/click-events/) 而不是射线投射。


以下示例使用 `engine.CameraEntity` 实体，从玩家的摄像机位置向前追踪一条射线。

```ts
raycastSystem.registerGlobalDirectionRaycast(
  {
    entity: engine.CameraEntity,
    opts: {
      queryType: RaycastQueryType.RQT_HIT_FIRST,
      direction: Vector3.rotate(
        Vector3.Forward(),
        Transform.get(engine.CameraEntity).rotation
      ),
    },
  },
  function (raycastResult) {
    console.log(raycastResult)
  }
)
```


> [!WARNING]
> **📔 注意**：请记住，在第三人称中，未来光标的行为可能不会与第一人称相同。建议仅在玩家处于第一人称时使用此方式。


## 从光标位置发出的射线投射

你也可以从玩家的光标位置向 3D 世界中追踪射线。这可用于拖动物体、射击游戏等。

在此示例中，我们检测玩家何时按下 E 键，然后从光标位置向 3D 世界中追踪一条射线。接着我们检查该射线是否击中任何实体，如果击中了，就对其执行某些操作。

```ts
import { engine, Entity, InputAction, inputSystem, PointerEventType, RaycastQueryType, raycastSystem, TextShape, Transform } from '@dcl/sdk/ecs'
import { PrimaryPointerInfo } from '@dcl/sdk/ecs'

let cooldown = 1
let rayFrequency = 0.1
let mousePressed = false

export function main() {
   engine.addSystem(rayCastSystem)
}

const rayCastSystem = (t: number) => {

    if (inputSystem.isTriggered(InputAction.IA_PRIMARY, PointerEventType.PET_DOWN)) {
      mousePressed = true
    }

    if (inputSystem.isTriggered(InputAction.IA_PRIMARY, PointerEventType.PET_UP)) {
      mousePressed = false
    }

    if (!mousePressed) {
      cooldown = 0
      raycastSystem.removeRaycasterEntity(engine.CameraEntity)
      return
    }

    cooldown += t
    if (cooldown < rayFrequency) return
    cooldown = 0

    const pointerInfo = PrimaryPointerInfo.getOrCreateMutable(engine.RootEntity)
    let dir = pointerInfo.worldRayDirection

    raycastSystem.registerGlobalDirectionRaycast(
      {
        entity: engine.CameraEntity,
        opts: {
          queryType: RaycastQueryType.RQT_HIT_FIRST,
          direction: dir,
        },
      },
      function (raycastResult) {
        let result = raycastResult.hits[0]

        // 在命中位置执行某些操作
        if (result && result.position) {
          console.log("x:", result.position.x, ", y:", result.position.y, ", z:", result.position.z)
        }

        // 对命中的实体执行某些操作
        const entity = result.entityId as Entity
        if (entity) {
          console.log("entity: ", entity)
        }
      }
    )
}

```


> [!NOTE]
> **💡 提示**：在此示例中，我们使用主按钮（E）来触发射线投射。我们不使用指针按钮（左键），因为点击并拖动时默认也会改变摄像机角度。如果你想在拖动时防止摄像机旋转，可以使用一个 [虚拟摄像机](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/camera/) 将摄像机角度设为固定的


## 高级语法

### 创建一个射线投射组件

Raycast 组件描述了用于查询相交实体的不可见射线。射线从实体的位置开始追踪，该位置由 Transform 组件定义，并受任何父实体的影响。方向可以通过多种方式定义，

射线使用以下数据定义：

* `方向`：一个包含 `$case` 字段，用于选择方向类型，以及一个取决于该类型的附加字段，用于确定该方向。以下是可接受的 `$case`:
  * `'localDirection'`：相对于实体正前方方向的方向，也会受到任何父实体变换的影响。这对于检测车辆前方的障碍物并保持其朝向很有用。旋转由 `localDirection` 字段定义，作为一个 `Vector3` ，它描述一个旋转。
  * `'globalDirection'`：忽略实体的旋转，并让其朝向某个方向，就像实体旋转为 0 一样。这例如适用于始终朝下。旋转由 `globalDirection` 字段定义，作为一个 `Vector3` ，它描述一个旋转。
  * `'globalTarget'`：在实体的位置与场景中的一个目标全局位置之间追踪一条线。它会忽略实体的旋转。可用于制作塔防游戏，每座塔的炮塔都可以指向空间中的一个精确坐标。目标由 `globalTarget` 字段定义，作为一个 `Vector3` ，它描述全局位置。
  * `'targetEntity'`：在实体的位置与第二个目标实体的位置之间追踪一条线。它会忽略任一实体的旋转。目标由 `targetEntity` 字段定义，其中保存了对该实体的引用。
* `maxDistance`: *数字* 用于设置这条射线的追踪长度。
* `queryType`: *RaycastQueryType* 枚举值，用于定义射线是返回所有命中的实体还是只返回第一个。可用选项如下：
  * `RaycastQueryType.RQT_HIT_FIRST`：只返回从起点开始遇到的第一个命中实体。
  * `RaycastQueryType.RQT_QUERY_ALL`：返回所有命中的实体，从起点一直到射线的最大距离。
* `collisionMask`：仅检测与特定碰撞层的碰撞。将其与自定义碰撞层一起使用，或者仅检测物理层或指针事件层。参见 [碰撞层](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/colliders/#collision-layers)。默认值为 `ColliderLayer.CL_POINTER | ColliderLayer.CL_PHYSICS`.
* `originOffset`：不要从实体的原点位置开始射线投射，而是添加一个偏移量，从相对位置开始查询。例如，你可以使用一个小偏移量来防止射线与实体自身的 3D 模型发生碰撞。
* `continuous`：如果为 true，则会在每一帧持续运行射线投射查询。如果为 false，则射线只会用于当前帧。默认情况下该值为 false。


> [!WARNING]
> **📔 注意**： `continuous` 属性应谨慎使用，因为每一帧运行射线投射查询的性能开销可能很大。在可能的情况下，使用一个 system（或 Utils 库中的 `interval` 函数）以更稀疏的固定间隔运行射线投射查询，参见 [循环射线投射](#recurrent-raycasting).


以下示例使用全局旋转来确定方向，并且只返回射线发出那一帧中被击中的第一个实体。

```typescript
const entity1 = engine.addEntity()

Transform.create(entity1, {
  position: Vector3.create(8, 1, 0)
})

Raycast.createOrReplace(entity1, {
  direction: {
    $case: "globalDirection",
    globalDirection: Vector3.create(0, 0, 1)
  },
  maxDistance: 16,
  queryType: RaycastQueryType.RQT_HIT_FIRST
})
```

下面的示例会沿着实体正前方的方向发射一条射线，只返回命中的第一个物体。它会持续执行。它还包含 0.5 的小偏移量，以防止射线击中实体自身的碰撞体。

```typescript
const entity1 = engine.addEntity()

Transform.create(entity1, {
  position: Vector3.create(8, 1, 0)
})

Raycast.createOrReplace(entity1, {
  direction: {
    $case: "localDirection",
    localDirection: Vector3.Forward()
  },
  maxDistance: 16,
  queryType: RaycastQueryType.RQT_HIT_FIRST,
  originOffset: Vector3.create(0.5, 0, 0),
  continuous: true
})
```

此示例在两个实体之间追踪一条射线。它会返回中间所有被击中的实体。

```ts
const entity1 = engine.addEntity()

Transform.create(entity1, {
  position: Vector3.create(8, 1, 0)
})

const entity2 = engine.addEntity()

Transform.create(entity2, {
  position: Vector3.create(0, 1, 8)
})

Raycast.createOrReplace(entity1, {
  direction: {
    $case: "targetEntity",
    targetEntity: entity2
  },
  maxDistance: 16,
  queryType: RaycastQueryType.RQT_QUERY_ALL
})
```

### Raycast 结果组件


> [!WARNING]
> **📔 注意**：处理 raycast 结果的最简单方法是使用 `raycastSystem`，并在创建射线的同一语句中注册一个回调函数。`RaycastResult` 组件在内部由该接口使用，同时也会公开出来，以支持更高级的自定义逻辑。


创建 Raycast 组件后，添加了该组件的实体将拥有一个 `RaycastResult` 组件。此组件包含关于射线任何命中的信息。请设置一个系统来检查这些数据。

该 `RaycastResult` 组件包含以下数据：

* `globalOrigin`：射线起始的位置，相对于场景。
* `方向`：射线指向的全局方向，作为一个 `Vector3`.
* `hits`：一个数组，其中每个被击中的实体对应一个对象。如果没有命中的实体，则此数组为空。如果射线投射使用了 `RaycastQueryType.RQT_HIT_FIRST`，那么该数组只会包含一个对象。

该 `hits` 数组中的每个对象都包含：

* `entityId`：被射线击中的实体的 ID 编号。
* `meshName`: *字符串* ，其中包含被击中的 3D 模型中具体网格的内部名称。当一个 3D 模型由多个网格组成时，这很有用。
* `位置`: *Vector3* 射线与命中实体相交的位置（相对于场景）
* `length`：射线从起点到与实体发生命中的位置之间的长度。
* `normalHit`: *Vector3* 用于表示世界空间中命中表面的法线。
* `globalOrigin`: *Vector3* 射线起始位置（相对于场景）
* `方向`：射线指向的全局方向，作为一个 `Vector3`.

下面的示例展示了如何使用系统访问单个实体的结果：

```typescript

const rayEntity = engine.addEntity()

Transform.create(rayEntity, {
  position: Vector3.create(8, 1, 0)
})

// 返回所有实体
Raycast.createOrReplace(rayEntity, {
  direction: {
    $case: "globalDirection",
    globalDirection: Vector3.create(0, 0, 1)
  },
  maxDistance: 16,
  queryType: RaycastQueryType.RQT_QUERY_ALL
})

engine.addSystem(() => {
  const rayResult = RaycastResult.get(rayEntity)
  console.log(rayResult.hits)
})
```

下一个示例展示了如何使用 `RaycastResult` 组件查询来访问场景中所有实体的 [组件](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/querying-components/).

```typescript
engine.addSystem(() => {
  for (const [_, result] of engine.getEntitiesWith(RaycastResult)) {
    console.log(result.hits)
  }
})
```


> [!WARNING]
> **📔 注意**：raycast 的结果不会在你创建 raycast 的同一游戏循环 tick 中到达。结果可能需要一个或多个 tick 才会到达。


在一个场景中，如果你为不同目的使用多种射线（例如路径查找、视线检查、弹道追踪等），你可能希望使用不同的 [碰撞层](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/colliders/#collision-layers)，以避免计算无关的碰撞。


> [!NOTE]
> **💡 提示**：有关 raycasting 的可运行示例，请参阅 [`77,-1-raycast-unit-tests`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/77,-1-raycast-unit-tests) 测试场景，该场景通过 `@dcl/sdk/testing` 中的单元测试来断言 raycast 行为，涵盖 `localDirection`, `globalDirection`, `globalTarget` 和 `targetEntity` 方向形式，包括从已变换父对象发射的射线。要查看射线可以命中的碰撞层如何表现，请参阅 [`5,5-collider-layers`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/5,5-collider-layers).



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/raycasting.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
