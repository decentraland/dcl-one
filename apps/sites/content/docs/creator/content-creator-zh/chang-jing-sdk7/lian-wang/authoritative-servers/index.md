> 使用无头多人服务器构建多人 Decentraland 场景。

# 多人服务器

**📔 注意**: 多人服务器以前称为 **权威服务器**。只有名称改变了，功能保持不变。要安装的 SDK 分支仍然名为 `auth-server`，参见 [设置](#setup).

## 概览

Decentraland 在玩家的机器上本地运行场景。默认情况下，玩家可以彼此看见并直接交互，但每个人都是独立地与环境交互。默认情况下，环境中的更改不会在玩家之间共享。

让所有玩家都能将一个场景视为具有相同内容、处于相同状态，这对于玩家进行更有意义的交互极其重要。如果没有这一点，若某个玩家打开了一扇门并走进房子，其他玩家会看到那扇门仍然是关着的，而第一个玩家在他们看来会像是直接穿过了那扇关着的门。

该 **多人服务器** 是一个无头服务器进程，会运行你的场景代码、验证状态更改，并将结果广播给所有已连接的玩家。它遵循权威服务器架构：不是信任每个客户端去报告自己的动作，而是由服务器作为唯一的事实来源。这使它成为同步多人场景的推荐方案。

只要游戏机制对公平性有要求，多人服务器就是理想选择，因为你可以实现在服务器端运行的复杂反作弊验证。你还可以在服务器上存储私钥和其他敏感信息，避免直接暴露给用户。

拥有多人服务器还能解决一个现实问题：在点对点架构中，两个玩家同时控制某个漂浮平台之类的东西，可能会产生冲突的结果。每个客户端都会把平台设置为不同的高度，而没有任何一方拥有决定哪个是正确的权威。多人服务器会在同一个地方解决每一次更改，因此所有客户端都会收敛到同一个状态。

它还为你提供了一个地方来 **在会话之间持久化数据**：排行榜、玩家进度、已解锁成就，或者像门被打开、物品被放置这类环境更改。玩家回来时，世界会反映之前发生过的事情。

Decentraland 会为你托管并部署服务器。通过正常流程发布你的场景，也会无缝发布服务器，无需额外步骤，也不需要为任何托管付费。

## 设置

### 1. 安装 auth-server 版 SDK

原生多人服务器 API（`isServer`, `registerMessages`, `Storage`, `EnvVar`等）可在单独的 SDK 分支中使用。请运行以下命令，将其安装到你的项目中，而不是标准 SDK 分支：

```bash
npm install @dcl/sdk@auth-server
npm install @dcl/js-runtime@auth-server
```

### 2. 配置 scene.json

多人服务器由标志 `"authoritativeMultiplayer": true` 在 `scene.json`中的根级别启用。你不需要手动添加它：当使用 SDK 的 auth-server 分支时，它会在你第一次构建或预览场景时自动添加。只要确保不要把它删掉；没有这个标志，服务器就不会运行，并且 `isServer()` 总是返回 *false*.

可选地，你也可以将以下内容添加到你的 `scene.json` 的根级别：

```json
{
  "logsPermissions": ["0xYourWalletAddress"]
}
```

添加 `logsPermissions` ，以便这些钱包在部署后能够访问服务器日志和指标。场景所有者和符合条件的部署协作者也可以访问，Genesis City 所有者以及该场景基础地块的委托 LAND 管理员也同样可以访问。参见 [场景洞察](/docs/creator/content-creator-zh/chang-jing-sdk7/tiao-shi/scene-insights/) 以了解权限、登录和命令。

### 3. 运行预览

使用标准预览命令，无需额外步骤。使用 auth-server 分支的 SDK 时，预览会在后台自动启动一个本地版本的多人服务器。

服务器的本地会话不会连接到生产环境中的会话，所以你可以放心测试，而不会影响你已发布场景中的玩家。

## 服务器 / 客户端分支

你项目中的场景代码位于 `src` 文件夹中，会同时在服务器和客户端运行。使用 `isServer()` 函数来分割执行路径：

```typescript
import { isServer } from "@dcl/sdk/network"
import { initServer } from "./server/server"
import { initClient } from "./client/setup"
import { setupUi } from "./client/ui"

function main() {
  if (isServer()) {
    // 仅服务器：游戏逻辑、验证、状态管理
    initServer()
    return
  } else {
    // 仅客户端：UI、输入处理、消息发送
    initClient()
    setupUi()
  }
}
```

服务器无渲染地以无头方式运行你的场景。它通过 `PlayerIdentityData` 对所有玩家位置、穿戴物和其他数据拥有经过验证的访问权限，并且是游戏状态的唯一权威。

## 同步组件与验证

### 将实体同步到所有客户端

使用 `syncEntity` 来广播该实体中指定组件的任何更改：

```typescript
import { isServer, syncEntity } from "@dcl/sdk/network"

if (isServer()) {
  syncEntity(
    entity,
    [Transform.componentId, GameState.componentId],
    /* enumId */ 1
  )
}
```

语法与以下功能使用的完全相同： [无服务器多人模式](/docs/creator/content-creator-zh/chang-jing-sdk7/lian-wang/serverless-multiplayer/) 功能，这使得将一个使用这种架构的场景升级为多人服务器变得非常容易。当场景使用多人服务器时，状态更新不再在所有玩家之间发送，而是改为通过服务器路由和验证所有状态更新。


> [!WARNING]
> **📔 注意**：使用多人服务器时，理想的模式是只让服务器调用 `syncEntity`。这样你就不必担心 entity-id 一致性问题。相反，实体由服务器实例化并共享，所有客户端都会同步到该实例。始终用 `isServer()`保护它。这与 [无服务器多人模式](/docs/creator/content-creator-zh/chang-jing-sdk7/lian-wang/serverless-multiplayer/)不同，在那种情况下，每个客户端都会自行调用 `syncEntity` 。


### 验证更改

使用 `validateBeforeChange()` 来限制实体某个组件中的任何状态更新。它允许你运行自定义验证函数，只有在验证测试通过时更改才会成功。

如果验证返回值 *true*，则更改被接受并传播给所有玩家。如果验证返回值 *false*，则更改被拒绝。被拒绝的更改不会传递给其他玩家，并且会在尝试进行更改的玩家那里被回滚。

#### 验证值

最简单的情况是验证新写入的 *值* 是否在某些参数范围内。例如，只接受对某个 `Transform` 的更改，当新的 Y 位置大于 0 时：

```typescript
import { engine, Transform } from "@dcl/sdk/ecs"
import { Vector3 } from "@dcl/sdk/math"
import { isServer } from "@dcl/sdk/network"

const entity = engine.addEntity()
Transform.create(entity, { position: Vector3.create(10, 2, 10) })

if (isServer()) {
  Transform.validateBeforeChange(entity, (value) => {
    // 拒绝任何会将实体放在 Y = 0 处或以下的更新
    return value.newValue.position.y > 0
  })
}
```

因为 `validateBeforeChange()` 仅在服务器上有意义，所以始终用 `isServer()`保护它。在客户端，这个调用不会产生任何有用的效果。

你可以用它作为反作弊机制，阻止那些违背游戏逻辑的更改。

#### 验证与玩家的距离

你也可以将 `validateBeforeChange()` 与服务器验证过的玩家位置结合起来，在允许玩家与某个对象交互之前，检查其是否足够接近。例如，当玩家尝试通过更改其 `Transform`来拾取一个对象时，如果该对象距离玩家超过 5 米，你可以拒绝这次更改：

```typescript
import { engine, Transform, PlayerIdentityData } from "@dcl/sdk/ecs"
import { Vector3 } from "@dcl/sdk/math"
import { isServer } from "@dcl/sdk/network"

const pickableEntity = engine.addEntity()
Transform.create(pickableEntity, { position: Vector3.create(8, 1, 8) })

if (isServer()) {
  Transform.validateBeforeChange(pickableEntity, (value) => {
    // 找到发出这次更改的玩家
    for (const [playerEntity, identity] of engine.getEntitiesWith(
      PlayerIdentityData
    )) {
      if (identity.address.toLowerCase() !== value.senderAddress.toLowerCase())
        continue

      const playerTransform = Transform.getOrNull(playerEntity)
      if (!playerTransform) return false

      // 在更改前获取对象的当前位置
      const objectTransform = Transform.getOrNull(pickableEntity)
      if (!objectTransform) return false

      const distance = Vector3.distance(
        playerTransform.position,
        objectTransform.position
      )

      // 只有当玩家在 5 米范围内时才允许更改
      return distance <= 5
    }

    // 在已连接玩家中未找到发送者——拒绝
    return false
  })
}
```

这种模式作为反作弊机制非常有用：它可以阻止玩家跨越整个场景去抓取他们本不应能够交互的对象。

#### 仅允许管理员进行更改

你也可以基于 *谁* 在发送更改来进行验证。每个传入值都包含一个 `senderAddress` 字段，里面是发送者的钱包地址。利用它仅允许某些玩家进行更改。例如，只允许场景管理员修改一个 `VideoPlayer` 组件：

```typescript
import { engine, VideoPlayer } from "@dcl/sdk/ecs"
import { isServer } from "@dcl/sdk/network"
import { isPreview } from "@dcl/asset-packs/dist/admin-toolkit-ui/fetch-utils"
import { getSceneAdmins } from "@dcl/asset-packs/dist/admin-toolkit-ui/ModerationControl/api"

// 从场景管理员列表刷新得到的管理员钱包地址缓存
let adminAddresses = new Set<string>()

async function updateAdminAddresses() {
  if (isPreview()) return
  try {
    const [error, response] = await getSceneAdmins()
    if (error) {
      console.error("[SERVER] 获取管理员列表时出错：", error)
      adminAddresses = new Set()
      return
    }
    adminAddresses = new Set(
      (response ?? []).map((admin) => admin.admin.toLowerCase())
    )
    console.log(
      "[SERVER] 已更新管理员地址缓存：",
      Array.from(adminAddresses)
    )
  } catch (error) {
    console.error("[SERVER] 更新管理员地址时出错：", error)
    adminAddresses = new Set()
  }
}

export async function main() {
  const videoEntity = engine.addEntity()
  VideoPlayer.create(videoEntity, { src: "videos/intro.mp4" })

  if (isServer()) {
    // 在设置验证之前填充缓存
    await updateAdminAddresses()

    VideoPlayer.validateBeforeChange(videoEntity, (value) => {
      // 在预览中始终允许更改，这样本地测试会更容易
      if (isPreview()) return true

      const senderAddress = value.senderAddress.toLowerCase()
      if (!adminAddresses.has(senderAddress)) {
        console.log(
          "[SERVER] 已阻止来自以下地址的未授权 VideoPlayer 更改：",
          senderAddress
        )
        return false
      }
      return true
    })
  }
}
```

参见 [场景管理员](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/shi-shi-yun-ying/scene-admin/) ，了解玩家如何在场景中成为管理员的更多上下文。

#### 发送意图，而非结果

每当分数、奖励或共享资源涉及其中时，都应围绕玩家 **想要做什么**来设计你的客户端消息，而不是围绕结果。客户端发送 `claimPoint` 或 `harvest`。它绝不会发送“我的分数现在是 40”。

然后服务器检查消息：是谁发送的、游戏是否处于允许这样做的阶段、玩家是否足够接近，以及是否还有可领取的内容。只有在那之后，它才会计算新值并写入。服务器是游戏状态的唯一写入者。

一个可以给自己的分数命名的客户端，就能给任何分数命名。

#### 仅允许由服务器进行更改

最严格的情况是只接受来自服务器自身的写入，拒绝来自客户端的任何更改。这是用于应当完全权威的状态的首选模式：分数、游戏阶段、生成的实体等。

每个传入值都包含一个 `senderAddress` 字段。发送者是服务器时，该字段与常量 `AUTH_SERVER_PEER_ID`相匹配，该常量导出自 `@dcl/sdk/network/message-bus-sync`.

下面的示例定义了一个小型 `protectServerEntity()` 辅助函数，用于对给定实体上的一个或多个组件应用此检查。这是一种方便的方法，可以在一次调用中保护多个组件（如 `Transform` 和 `GltfContainer`）:

```typescript
import { engine, Entity, Transform, GltfContainer } from "@dcl/sdk/ecs"
import { Vector3 } from "@dcl/sdk/math"
import { isServer } from "@dcl/sdk/network"
import { AUTH_SERVER_PEER_ID } from "@dcl/sdk/network/message-bus-sync"

type ComponentWithValidation = {
  validateBeforeChange: (
    entity: Entity,
    cb: (value: { senderAddress: string }) => boolean
  ) => void
}

function protectServerEntity(
  entity: Entity,
  components: ComponentWithValidation[]
) {
  for (const component of components) {
    component.validateBeforeChange(entity, (value) => {
      return value.senderAddress === AUTH_SERVER_PEER_ID
    })
  }
}

if (isServer()) {
  // 创建服务器管理的实体后：
  const entity = engine.addEntity()
  Transform.create(entity, { position: Vector3.create(10, 5, 10) })
  GltfContainer.create(entity, { src: "assets/model.glb" })
  protectServerEntity(entity, [Transform, GltfContainer])
}
```


> [!WARNING]
> **📔 注意**：始终在 `protectServerEntity()` 内部调用 `isServer()` 代码块。它包装了 `validateBeforeChange()`，这只有在服务器上才有意义——在客户端调用会产生错误。


#### 自定义组件

你也可以将 `validateBeforeChange()` 应用于场景定义的自定义组件。

```typescript
import { engine, Schemas } from "@dcl/sdk/ecs"
import { isServer } from "@dcl/sdk/network"
import { AUTH_SERVER_PEER_ID } from "@dcl/sdk/network/message-bus-sync"

export const GameState = engine.defineComponent("game:State", {
  phase: Schemas.String,
  score: Schemas.Int,
  timeRemaining: Schemas.Int,
})

// 只有服务器可以修改这个组件
if (isServer()) {
  GameState.validateBeforeChange((value) => {
    return value.senderAddress === AUTH_SERVER_PEER_ID
  })
}
```

## 消息

同步组件非常适合所有玩家都应持续看到的状态：例如位置、分数或游戏阶段。但并非所有内容都符合这种模型。有时玩家只需要告诉服务器“我点了这个按钮”或“我想加入游戏”，而服务器需要以一次性响应来回复，例如“回合开始了”或“这是你的统计信息”。这些是事件，而不是持续状态。

这正是消息的用途。使用 `registerMessages()` 在客户端和服务器之间进行类型化、由 schema 验证的通信。消息是一次发送，不负责状态持久化：客户端发送消息给服务器，服务器处理它，并可选择再发回一条。它们本身不会直接创建任何持久状态。

### 定义消息

在一个共享文件中定义所有消息，该文件由服务器和客户端共同导入。这样双方始终会就有哪些消息以及它们携带什么数据达成一致。每条消息都是一个 `Schemas.Map` ，用于描述其载荷：

```typescript
import { Schemas } from "@dcl/sdk/ecs"
import { registerMessages } from "@dcl/sdk/network"

export const Messages = {
  // 客户端 → 服务器
  playerReady: Schemas.Map({ displayName: Schemas.String }),
  playerAction: Schemas.Map({ action: Schemas.String, targetId: Schemas.Int }),
  giveCoins: Schemas.Map({ to: Schemas.String, amount: Schemas.Int }),

  // 服务器 → 客户端
  gameStarted: Schemas.Map({ roundNumber: Schemas.Int }),
  gameEnded: Schemas.Map({ winnerId: Schemas.String }),
}

export const room = registerMessages(Messages)
```


> [!WARNING]
> **📔 注意**：在 `registerMessages()` 共享模块的顶层调用一次，如上例所示。这样模块首次加载时，它会同时在服务器和客户端上运行。不要有条件地调用它，也不要在函数内部调用；双方必须始终注册相同的消息。


### 发送消息

客户端只能向服务器发送消息。不存在客户端之间的直接消息传递。服务器可以广播给所有客户端，也可以按地址针对特定玩家发送。

```typescript
import { room } from "./shared/messages"

// 客户端 → 服务器（广播，服务器接收）
room.send("playerReady", { displayName: "Alice" })

// 服务器 → 所有客户端
room.send("gameStarted", { roundNumber: 1 })

// 服务器 → 某个特定客户端（按钱包地址）
room.send("gameEnded", { winnerId: "Alice" }, { to: [playerAddress] })
```


> [!WARNING]
> **📔 注意**：消息的最大大小约为 13 KB。更大的消息会被传输层静默丢弃，不会产生任何错误。请保持载荷较小；如果你需要共享更大的数据，请把它拆分成多条消息，或者使用同步组件。


### 接收消息

```typescript
import { room } from "./shared/messages"

// 客户端从服务器接收
room.onMessage("gameStarted", (data) => {
  console.log(`第 ${data.roundNumber} 轮开始了！`)
})

// 服务器从客户端接收
room.onMessage("playerReady", (data, context) => {
  if (!context) return
  const senderAddress = context.from // 已验证的钱包地址
  console.log(`[Server] ${data.displayName} 已准备好 (${senderAddress})`)
})
```

在服务器上，每条接收到的消息都包含一个 `context` 对象，其中包含发送者已验证的钱包地址。利用它来知道是哪位玩家发送了消息（切勿依赖载荷中自报的身份）。

`context.from` 会严格按照网络报告的原样到达，不会更改大小写。只需将其转为小写一次（`context.from.toLowerCase()`），并将该值用于每个 map、set 和 Storage 调用，这样某个玩家的数据就不会被拆分到两个键中。

### 在发送前等待状态同步

客户端应等到场景状态同步完成后再发送第一条消息，以避免加入时发生竞态条件：

```typescript
import { engine } from "@dcl/sdk/ecs"
import { isStateSyncronized } from "@dcl/sdk/network"
import { room } from "./shared/messages"

engine.addSystem(() => {
  if (!isStateSyncronized()) return

  // 现在可以安全发送消息了
  room.send("playerReady", { displayName: "Alice" })
})
```


> [!NOTE]
> **💡 提示**: `isStateSyncronized()` 只会告诉你客户端状态已同步，并不能保证服务器已完成初始化。要做更稳健的就绪检查，可以让服务器广播一条“ready”或心跳消息，然后让客户端在发送游戏消息前等待它。


### 可用的 Schema 类型

所有消息载荷和自定义组件都使用 `Schemas` 进行二进制序列化。以下是可用类型的快速参考：

```typescript
import { Schemas } from "@dcl/sdk/ecs"

// 基本类型
Schemas.String // "你好"
Schemas.Int // 42
Schemas.Float // 3.14
Schemas.Boolean // true / false
Schemas.Int64 // Date.now()

// 向量类型
Schemas.Vector3 // { x: 1, y: 2, z: 3 }
Schemas.Quaternion // { x, y, z, w }

// 复杂类型
Schemas.Array(Schemas.String) // ["a", "b", "c"]
Schemas.Entity // 实体引用
Schemas.Optional(Schemas.String) // "hello" 或 undefined
Schemas.Optional(Schemas.Int) // 42 或 undefined

// 嵌套对象
Schemas.Map({
  name: Schemas.String,
  health: Schemas.Int,
  position: Schemas.Vector3,
  playerId: Schemas.Optional(Schemas.String),
})
```


> [!WARNING]
> **📔 注意**: 消息 *必须* 必须使用 `Schemas.Map(...)`。你不能发送普通的 JavaScript 对象，它们会在二进制序列化时失败。


## 服务器读取玩家位置

服务器可以读取 **经验证的** 玩家位置；客户端无法伪造这些。这是基于位置的反作弊基础：

```typescript
import { engine, PlayerIdentityData, Transform } from "@dcl/sdk/ecs"

engine.addSystem(() => {
  for (const [entity, identity] of engine.getEntitiesWith(PlayerIdentityData)) {
    const transform = Transform.getOrNull(entity)
    if (!transform) continue

    const address = identity.address
    const position = transform.position
    // 这个位置由服务器验证——绝不要信任客户端报告的位置
  }
})
```


> [!WARNING]
> **📔 注意**: 始终使用 `PlayerIdentityData` + `Transform` 在服务器上获取玩家位置。绝不要信任客户端自身报告的值。


## 数据存储

在服务器重启后持久保存数据。存储是 **仅服务器可用**，始终用……保护调用 `isServer()`。服务器既可以写入也可以读取这些数据。

```typescript
import { Storage } from "@dcl/sdk/server"
```

数据可以在两个层级中存储：

* **世界**：用于与所有玩家相关的数据，例如排行榜或持久化的环境变化。
* **玩家**：用于玩家专属数据，例如保存该玩家的进度或偏好。


> [!NOTE]
> **💡 提示**：存储值是 JSON。你可以原样存储数字、字符串、数组或普通对象，并以相同的值读回。你不需要 `JSON.stringify()` 或 `JSON.parse()`。有两个例外： `Date` 会作为其 ISO 文本存储（`"2026-10-27T13:05:12.000Z"`），而对象属性设为 `undefined` 会被丢弃。你之前用……保存的值 `JSON.stringify()` 是字符串，因此读回时仍是该字符串。
> 
> 在本地开发期间，存储会写入到 `node_modules/@dcl/sdk-commands/.runtime-data/server-storage.json`.



> [!WARNING]
> **📔 注意**：存储调用如何报告问题：
> 
> * `Storage.set()` 和 `Storage.player.set()` 会返回一个 **布尔值**. `false` 表示该值 **未** 被持久化，例如在网络错误之后，或者当同时运行的请求过多时（见下文）。始终检查结果：被丢弃的 `false` 就是一次静默丢失的保存。
> * `set()` 会抛出一个 `TypeError` 当该值无法原样作为 JSON 存储时： `undefined`, `NaN` 或 `Infinity`，函数，或 `Map` 或 `Set`等。要删除键，请使用 `delete()`.
> * `get()` 会解析为存储的值，或者解析为 `null` 仅在键不存在时。读取失败时会抛出，因此失败绝不会被误认为是缺失的键。
> * 该 `delete()` 变体在请求失败时会抛出。它们会解析为 `false` 仅当键不存在时。


### 在检查点保存，而不是每次变更都保存

存储是为必须在服务器重启和重新部署后仍然存在的数据提供的持久化手段。它不是一个实时数据存储。把你的游戏运行状态保存在服务器内存中。那样更快，也更符合服务器的正确模式。只有在真正需要时、在有意义的检查点才写入存储。


> [!WARNING]
> **⚠️ 警告**：服务器运行时最多允许 **40 个同时进行的主机调用** 同时进行，并在以下各项之间共享： *一切* 场景请求运行时执行的所有操作：每一次存储请求， `signedFetch`以及其他运行时 API，都会计入同一限制。超出的调用不会 **排队**。它们会立即以 `并发主机调用过多` 错误被拒绝。SDK 会捕获该拒绝，并将 `Storage.set` promise 解析为 `false` 而不是抛出。如果你的代码丢弃了那个布尔值，失败的保存就会不可见，而你的持久化数据最终会过期或丢失。检查每一次写入的结果（见上面的说明）。


适合持久化的时机：

* 游戏结束，或一轮结束。
* 有玩家离开。
* 定期的防抖保存（例如每 30 秒一次），而不是每帧一次。

只持久化必须在重启或重新部署后仍然存在的数据。其他一切都可以保存在内存中。

下面的示例将分数保存在内存中，只在检查点写入，而不是每得一分就写入：

```typescript
import { Storage } from "@dcl/sdk/server"

// 工作状态保存在内存中，并在每次事件时更新
const scores: Record<string, number> = {}

function onPointScored(address: string) {
  scores[address] = (scores[address] ?? 0) + 1
  // 这里不调用 Storage——只更新内存
}

// 仅在检查点持久化，例如玩家离开时
async function onPlayerLeave(address: string) {
  const saved = await Storage.player.set(address, "score", scores[address] ?? 0)
  if (!saved) {
    // 该写入未持久化——把值保留在内存中，稍后重试
    console.error(`未能为 ${address} 保存分数`)
  }
}
```

对于有大量并发玩家的场景，更稳健的方法是跟踪哪些键有未保存的更改（一个“脏集”），并在下一次刷新时重试失败的写入：

```typescript
import { engine } from "@dcl/sdk/ecs"
import { Storage } from "@dcl/sdk/server"

// 工作状态保存在内存中，并在每次事件时更新
const scores: Record<string, number> = {}
const dirty = new Set<string>()

function onPointScored(address: string) {
  scores[address] = (scores[address] ?? 0) + 1
  dirty.add(address) // 标记为稍后刷新，这里不调用 Storage
}

// 定期刷新脏键（例如每 ~30 秒）以及在检查点刷新
async function flush() {
  for (const address of dirty) {
    const ok = await Storage.player.set(address, "score", scores[address])
    if (ok) {
      dirty.delete(address) // 已成功保存
    }
    // 如果 ok 为 false，键会保持为脏状态，并在下一次刷新时重试
  }
}

// 使用 dt 累加器按计时器运行刷新
let flushTimer = 0
engine.addSystem((dt) => {
  flushTimer += dt
  if (flushTimer > 30) {
    flushTimer = 0
    flush()
  }
})
```

### 世界存储——所有玩家共享

```typescript
import { Storage } from "@dcl/sdk/server"

type Entry = { name: string; score: number }

// 写入——如果值未持久化，set() 会解析为 false
const ok = await Storage.set("leaderboard", [
  { name: "Alice", score: 100 },
  { name: "Bob", score: 85 },
])
if (!ok) {
  console.error("保存排行榜失败——稍后重试")
}

// 读取：键不存在时为 null
const leaderboard = (await Storage.get<Entry[]>("leaderboard")) ?? []

// 删除
await Storage.delete("leaderboard")
```

你也可以通过命令行管理场景存储，使用 `npx sdk-commands storage scene`:

```bash
# 设置一个值
npx sdk-commands storage scene set high_score --value 100

# 获取一个值
npx sdk-commands storage scene get high_score

# 删除一个值
npx sdk-commands storage scene delete high_score

# 删除所有场景存储数据
npx sdk-commands storage scene clear --confirm
```

### 玩家存储——按钱包地址

```typescript
import { Storage } from "@dcl/sdk/server"

type Progress = { level: number; coins: number }

// 写入——如果值未持久化，set() 会解析为 false
const ok = await Storage.player.set(playerAddress, "progress", {
  level: 5,
  coins: 250,
})
if (!ok) {
  console.error(`未能为 ${playerAddress} 保存进度——稍后重试`)
}

// 读取：当该玩家没有已保存的进度时为 null
const progress = (await Storage.player.get<Progress>(playerAddress, "progress")) ?? { level: 1, coins: 0 }

// 删除
await Storage.player.delete(playerAddress, "progress")
```

你也可以通过命令行管理玩家存储，使用 `npx sdk-commands storage player`:

```bash
# 为特定玩家设置一个值
npx sdk-commands storage player set level --value 10 --address 0x1234...

# 获取特定玩家的一个值
npx sdk-commands storage player get level --address 0x1234...

# 删除特定玩家的一个值
npx sdk-commands storage player delete level --address 0x1234...

# 删除特定玩家的所有数据
npx sdk-commands storage player clear --address 0x1234... --confirm

# 删除所有玩家数据（所有玩家）
npx sdk-commands storage player clear --confirm
```

每个场景值最多可达 512 KB，而一个世界的所有场景值共用 10 MB。每个玩家值最多可达 100 KB，而每个玩家共用 1 MB。

### 在不覆盖的情况下加载存档

当玩家加入时，绝不要先分配默认数据然后再保存：如果他们的已保存数据存在但尚未加载，默认值会将其替换。先读取，并且只在……时创建默认值 `get()` 解析为 `null`。如果读取抛出异常，就让该玩家保持未加载状态，并稍后再试。

```typescript
import { Storage } from "@dcl/sdk/server"

type Save = { coins: number }
const saves: Record<string, Save> = {}

async function loadOrCreate(address: string): Promise<Save> {
  if (saves[address]) return saves[address] // 已加载
  // 如果读取失败会抛出，因此失败绝不会变成一个新的存档
  const stored = await Storage.player.get<Save>(address, "save")
  saves[address] = stored ?? { coins: 0 } // 仅在不存在存档时使用默认值
  return saves[address]
}
```

在……中调用它 `try/catch` 在你的加入处理器中。如果它抛出异常，就通知客户端以便重试，并在该玩家的存档加载完成之前忽略其操作。

### 场景中没有人时的进度

不要为随时间增长的事物保持计时器运行，例如作物、能量或商店补货。多人服务器会在最后一名玩家离开后几分钟停止（见 [等待服务器启动](#wait-for-the-server-to-start-up)），所以计时器也会停止。相反，存储进度最后更新时间。当玩家返回时，根据经过的时间计算进度，并向前推进存储的时间。

使用服务器的 `Date.now()` 来处理这个，绝不要使用客户端发送的时间：玩家可以把自己的时钟向前调来跳过等待。

### 避免在并发写入下丢失数据

存储没有办法仅在你读到它之后没人更改过时才更新一个键（没有“比较并设置”）。所以如果你读取一个键、修改值再写回，你就可能丢失数据：

* 两个服务器实例可以同时写入。这发生在重新部署期间，你的场景的旧版本和新版本重叠时。
* 一次读取返回的值可能比最后写入的更旧。即使使用 `Storage.get(key, { fresh: true })`，以及跨服务器重启时也是如此。 `fresh` 选项也只会跳过 SDK 自己的缓存。
* 使用……获取的列表也可能 `Storage.getValues()` 会过期：它可能遗漏最近写入的键，或者仍然显示你已删除的键。

对于只会增长的数据，例如结果日志或玩家反馈，请使用这种模式，而不是重写同一个键：

1. 将每个快照写入一个 **新键** ，这个键按时间排序且不会冲突，例如 `results-1-20261027-130512-k3fz`：一个前缀、一个分段编号、UTC 时间和一些随机字符。
2. 读取时，使用……列出该分段中的每个键 `Storage.getValues({ prefix, offset })`，逐页读取，直到你拥有 `pagination.total` 条目，然后按行 id 合并它们。
3. 将合并后的行写成一个新键。把该键读回，只有在它匹配时，才删除你合并的那些键。绝不要删除一个其他写入者仍可能基于你自己的读取结果继续更改的键。

这样，竞态会留下一个额外的键，下一次写入会将其合并，而不是丢失一行。任何读取这些数据的人，包括你从……下载它时 [存储 UI](#access-stored-data)，都必须读取该分段中的每个键并丢弃重复的 id。

```typescript
import { Storage } from "@dcl/sdk/server"

type Row = { id: string; text: string }
const PREFIX = "results-1-"

// 每个以该前缀开头的键，按最旧在前
async function listAll(prefix: string) {
  const entries: { key: string; value: unknown }[] = []
  for (;;) {
    const page = await Storage.getValues({ prefix, offset: entries.length })
    entries.push(...page.data)
    if (page.data.length === 0 || entries.length >= page.pagination.total) break
  }
  return entries.sort((a, b) => (a.key < b.key ? -1 : 1))
}

async function appendRows(newRows: Row[]): Promise<boolean> {
  const versions = await listAll(PREFIX)
  // 按行 id 合并，先看到的优先
  const rows = new Map<string, Row>()
  for (const { value } of versions) for (const row of value as Row[]) if (!rows.has(row.id)) rows.set(row.id, row)
  for (const row of newRows) if (!rows.has(row.id)) rows.set(row.id, row)
  const merged = [...rows.values()]

  // 每次都使用新键：UTC 时间加随机字符
  // “2026-10-27T13:05:12.123Z” 变成 “20261027-130512”
  const stamp = new Date().toISOString().slice(0, 19).replace(/[-:]/g, "").replace("T", "-")
  const key = `${PREFIX}${stamp}-${Math.random().toString(36).slice(2, 6)}`
  if (!(await Storage.set(key, merged))) return false // 未保存，稍后重试

  // 只有在新键读回成功后，才删除已合并的键
  const check = await Storage.get<Row[]>(key, { fresh: true })
  if (JSON.stringify(check) !== JSON.stringify(merged)) return true
  for (const version of versions) {
    try {
      await Storage.delete(version.key)
    } catch {
      // 保留在原处：下一次写入会再次合并它
    }
  }
  return true
}
```

一个值最多只能容纳 512 KB，因此请在一个分段快接近该大小时开始新的分段（`results-2-`）以免某个分段接近上限。 [场景反馈示例](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/0%2C-1-scene-feedback) 使用这种模式处理其 CSV 文件，并在 400 KB 时切换到新的分段。

### 访问已存储的数据

你可以通过存储 UI 在服务器上查看和编辑实时存储的数据，只需打开此链接：

[decentraland.org/storage](https://decentraland.org/storage)

你也可以通过 Creator Hub 打开此页面。打开 **管理** 选项卡，点击你已发布内容的位置旁边的三个点，然后选择 **查看存储**.

在那里，你可以看到所有可以发布场景的世界和土地列表。

打开你的场景，然后打开 **场景** 或 **玩家** 选项卡。

在 **场景** 选项卡中，你会看到所有已存储变量的列表。在这里，你可以通过点击铅笔或垃圾桶图标来编辑或删除这些变量。

![Activate stream](/docs/_remote/76bca9abaef9d73bc207163d.webp)

在 **玩家** 选项卡中，你会看到所有在你的服务器上存有任何数据的玩家列表。你可以按地址或名称搜索他们，然后查看其关联的所有数据。你也可以通过点击铅笔或垃圾桶图标来编辑或删除这些数据。

### 更改数据结构

生产环境中的存储数据 **在你发布场景新版本时不会被清除**。这对排行榜、玩家进度以及玩家期望在场景小更新之外仍然保留的持久环境变化非常有用。

反过来说，存储中的数据是由旧版本代码写入的。如果你的新代码期望不同的结构，那么解析或读取这些旧数据可能会以微妙的方式失败。你重命名过的字段会缺失。一个过去是字符串、现在变成对象的字段，在你尝试访问其属性时会抛出错误。一个几个月没登录的玩家，可能会加载出比你的代码已不再知道如何处理的结构更早的数据。


> [!WARNING]
> **📔 注意**：模式更改并不只会影响部署后的第一次读取。存储的数据会一直存在，直到被覆盖或删除，因此旧格式的值可能随时出现，而且通常来自你已忘记的回归玩家。


#### 最佳实践

* *始终以防御性方式读取*。把从存储中读出的任何内容都当作不可信输入，尽管它是你自己写入的。你传给 `get<T>()` 的类型不会被检查，所以在读取之前先检查字段是否存在以及类型是否正确，并在它们不存在时准备一个合理的默认值：

  ```typescript
  import { Storage } from "@dcl/sdk/server"

  const saved = await Storage.player.get<any>(playerAddress, "progress")
  const progress = {
    level: typeof saved?.level === "number" ? saved.level : 1,
    coins: typeof saved?.coins === "number" ? saved.coins : 0,
  }
  ```
* *添加字段，不要重命名或删除字段*。最安全的模式变更是追加式的：引入一个带默认值的新字段，并保持现有字段不变。旧数据只是缺少新字段，而你的防御性解析已经能处理这一点。重命名字段会迫使每条旧记录失效。
* *为存储对象添加版本*。包含一个 `版本` 字段，从一开始就加上。读取数据时，根据版本分支，并在使用之前将旧结构迁移到当前结构。这能让你的其余代码始终只处理一种当前结构：

  ```typescript
  type ProgressV2 = { version: 2; level: number; coins: number; xp: number }

  function migrate(raw: any): ProgressV2 {
    const version = raw?.version ?? 1
    if (version === 1) {
      // v1 没有 xp 字段——给它一个默认值
      return {
        version: 2,
        level: raw.level ?? 1,
        coins: raw.coins ?? 0,
        xp: 0,
      }
    }
    return raw as ProgressV2
  }
  ```
* *将迁移后的值写回*。一旦你在内存中升级了某条记录，就把它保存回去，这样下一次读取时已经是新格式。随着时间推移，这会逐步清空旧结构记录池，而无需一次性迁移脚本。
* *对于破坏性变更，请使用新键*。如果新结构确实不兼容，而且迁移不值得，那么请写入一个新的存储键（例如 `progress_v2`) 并忽略旧的那个。旧键会安静地留在存储中，不会造成影响，而且你可以避免任何需要解析它的读取路径。你之后可以通过 [存储 UI](https://decentraland.org/storage) 或 `npx sdk-commands storage` 命令清理旧键。
* *使用真实生产数据进行测试*。在部署结构性变更之前，先从存储 UI 中拉取几条真实记录，并用你的新解析代码对它们运行一遍。真正会引发问题的边缘情况，通常都是你不知道自己还存在的记录。
* *保留一个后门*。请记住，你可以从存储 UI 中，或通过 `npx sdk-commands storage`编辑或删除单条记录。如果在架构变更后，某个玩家陷入了坏状态，你可以直接修复它的记录，而无需重新部署。

## 环境变量

通过环境变量来配置你的场景，而不是把值硬编码到代码里。环境变量适合存放敏感数据，也适合存放特性开关或可以在不重新发布场景的情况下轻松修改的参数。

环境变量是 **仅服务器可用**。请用 `isServer()`来保护它们。服务器可以读取环境变量，但不能修改它们的值。

`EnvVar.get()` 返回一个 `Promise<string>` ，当变量未设置时会解析为空字符串，所以一定要为缺失值提供一个回退值：

```typescript
import { EnvVar } from "@dcl/sdk/server"
import { isServer } from "@dcl/sdk/network"

export async function main() {
  if (!isServer()) return

  const maxPlayers = parseInt((await EnvVar.get("MAX_PLAYERS")) || "4")
  const gameDuration = parseInt((await EnvVar.get("GAME_DURATION")) || "300")
  const debugMode = ((await EnvVar.get("DEBUG")) || "false") === "true"
}
```

### 敏感数据

环境变量尤其适合存放私钥、奖励领取码以及其他敏感数据，因为把这些内容暴露在公开场景的编译代码里会有风险。

你可以把私钥存放在服务器的存储中，并且只让服务器通过 `isServer()`读取它们。这样敏感数据就不会经过玩家的机器。

### 本地开发

要在本地运行项目时使用环境变量，请在你的项目根目录创建一个 `.env` 文件：

```
MAX_PLAYERS=8
GAME_DURATION=300
DEBUG=true
```

重要：请把 `.env` 添加到你的 `.gitignore`中，这样这些可能敏感的值就永远不会被上传到公开内容服务器。

### 更改环境变量

更改环境变量值最简单的方法是通过存储 UI。

你可以通过输入这个链接来访问场景存储中保存的数据：

[decentraland.org/storage](https://decentraland.org/storage)

你也可以通过 Creator Hub 打开此页面。打开 **管理** 选项卡，点击你已发布内容的位置旁边的三个点，然后选择 **查看存储**.

在那里，你可以看到所有可以发布场景的世界和土地列表。

打开你的场景，然后打开 **环境** 标签页。你应该能看到项目中的所有环境变量。

![Activate stream](/docs/_remote/13189fe9bb9d6d0cc3454293.webp)

请注意，你无法读取这些环境变量的值（这是为了保护敏感数据），但你可以删除或覆盖它们。只需点击铅笔或垃圾桶图标即可。

你也可以通过命令行管理环境变量，使用 `npx sdk-commands storage env`:

```bash
# 设置变量
npx sdk-commands storage env set MAX_PLAYERS --value 8

# 删除变量
npx sdk-commands storage env delete OLD_VAR

# 删除所有环境变量
npx sdk-commands storage env clear --confirm
```

你也可以使用 `--target` 标志来指定一个特定环境：

```bash
# 部署到预发布环境
npx sdk-commands storage env set MY_KEY --value my_value --target https://storage.decentraland.zone

# 部署到本地开发服务器
npx sdk-commands storage env set MY_KEY --value my_value --target http://localhost:8000
```

已部署的环境变量优先于 `.env` 值。

## 推荐的项目结构

将服务端、客户端和共享代码分离，能让代码库在增长时依然保持可读性：

```
src/
├── index.ts              # 入口点 — isServer() 分支
├── client/
│   ├── setup.ts          # 输入处理器、消息发送器
│   └── ui.tsx            # React ECS UI（读取同步状态）
├── server/
│   ├── server.ts         # 游戏循环、消息处理器、状态变更
│   └── gameState.ts      # 服务器状态的辅助函数
└── shared/
    ├── schemas.ts        # 组件定义 + validateBeforeChange
    └── messages.ts       # registerMessages() — 两端都会导入
```


> [!NOTE]
> **💡 提示**：将所有 `registerMessages()` 调用和自定义组件定义都放在 `shared/`中。服务器和客户端都从这里导入，确保它们始终就消息 schema 达成一致。


## 性能最佳实践

每次组件变更都会把 *整个* 组件数据通过网络发送出去。这与 Colyseus 的做法不同，后者只发送差异。在设计自定义组件时，要牢记这一点。最优解可能是根据变更频率，把数据存放在不同的组件中。

### ❌ 避免使用单体组件

```typescript
import { engine, Schemas } from "@dcl/sdk/ecs"

// 坏例子——更改分数也会发送位置数组
const GameState = engine.defineComponent("GameState", {
  playerAScore: Schemas.Int,
  playerBScore: Schemas.Int,
  timer: Schemas.Int,
  playerPositions: Schemas.Array(Schemas.Vector3), // 大负载
})
```

### ✅ 更偏好原子化组件

```typescript
import { engine, Schemas } from "@dcl/sdk/ecs"

// 好例子——每次更新都很小且相互独立
const PlayerScore = engine.defineComponent("PlayerScore", {
  playerA: Schemas.Int,
  playerB: Schemas.Int,
})

const GameTimer = engine.defineComponent("GameTimer", {
  secondsLeft: Schemas.Int,
})
```

*经验法则*：将那些会一起变化、且变化频率相近的字段分组。把变化快的数据（计时器、位置）与变化慢的数据（分数、配置）分开。

### 限制高频消息

避免在每一帧都发送消息。尽可能进行批处理或节流：

```typescript
import { engine } from "@dcl/sdk/ecs"
import { room } from "./shared/messages"

let lastSend = 0
engine.addSystem((dt) => {
  lastSend += dt
  if (lastSend > 0.1) {
    // 每 100 毫秒
    room.send("position", transform.position)
    lastSend = 0
  }
})
```

例如，如果服务器控制一个倒计时器，就没必要每秒向所有玩家发送更新。最好让每个客户端自己计算时间流逝，然后让服务器每隔 30 秒左右广播一次当前状态，以确保一致性。

## 服务器资源限制

Multiplayer Server 会在带有硬性资源上限的沙箱 isolate 中运行每个场景。触及这些限制可能会悄悄丢失数据，或者让场景中的所有人都遇到服务器终止，因此请把你的场景设计在这些限制之内留有充足余量。

### 内存

该 isolate 的 **256 MB** 内存上限。如果超出，isolate 会被销毁，服务器会对所有已连接玩家关闭。请保持工作状态精简，并在玩家离开时清理按玩家划分的数据。

### CPU

每个执行轮次都有一个实际时间预算：

* **同步执行**：每轮 10 秒。超过这个时间的无限循环会让 isolate 终止。
* **异步轮次结算**：60 秒。如果一个 `Promise` 链或 `await` 完成解析的时间超过这个限制，isolate 将被终止。

使用 `dt` 累加器，把大量工作分摊到多个 tick 中，放在 `engine.addSystem()`里。切勿在服务器上运行无限的同步循环。

### 入站消息速率

每个已连接对等端每 1,000 ms 大约最多可发送 **300 条消息**。超出的数据帧会被丢弃（不会排队）。切勿从客户端在每一帧都发送消息。参见 [性能最佳实践](#performance-best-practices) 了解节流模式。

### 消息大小

* 入站数据包上限为 **128 KB** 每个数据包。超大的数据包会被直接丢弃。
* 场景到通信消息的上限大约为 **30 KB**。对于实际传输层，请让同步消息远低于 **13 KB** （参见 [消息](#messages) 这一节）。

### 外部 fetch

并发 `signedFetch` 调用的上限为 **32** 个进行中。额外的 fetch 会排队，直到有空位。每次 fetch 尝试都有 **15 秒** 的超时。fetch 响应上限为 **10 MB**。WebSocket 连接受 **32** 个并发 socket 限制，单条消息最大为 **1 MB** 。

### 进行中的宿主调用

在 [数据存储](#data-storage) 中描述的 40 次调用上限适用于整个 isolate 范围内的所有宿主调用，包括 Storage、 `signedFetch`以及其他运行时 API。超出的调用会立即被拒绝，不会排队。

## 常见陷阱

### 忘记对仅服务端状态进行校验

如果没有 `validateBeforeChange`，客户端可以向任何组件写入：

```typescript
import { engine, Schemas } from "@dcl/sdk/ecs"
import { isServer } from "@dcl/sdk/network"
import { AUTH_SERVER_PEER_ID } from "@dcl/sdk/network/message-bus-sync"

// ❌ 坏例子——客户端可以作弊
const Score = engine.defineComponent("Score", { value: Schemas.Int })

// ✅ 好例子——仅限服务器
if (isServer()) {
  Score.validateBeforeChange((v) => v.senderAddress === AUTH_SERVER_PEER_ID)
}
```

### 信任客户端提供的值

绝不要让客户端替它自己决定健康值、分数或位置等重要数据：

```typescript
import { room } from "./shared/messages"

// ❌ 坏例子
room.onMessage("setHealth", (data) => {
  player.health = data.health // 客户端控制了这个值！
})

// ✅ 好例子——服务器计算结果
room.onMessage("takeDamage", (data) => {
  const damage = calculateDamage(data.source)
  player.health = Math.max(0, player.health - damage)
})
```

### 信任消息中的金额和地址

来自客户端的消息是请求，不是事实。在使用之前检查每个字段：

* **金额**: `Schemas.Int` 会将该值解码为 32 位整数，但它仍然可能是 0 或负数。拒绝 0 或更小的金额，否则玩家可能会“发送” `-500` 代币，然后反而收到它们。还要检查发送者是否有足够余额。
* **地址**：客户端发送的钱包地址必须是 `0x` 后跟 40 个十六进制字符。其余任何内容都要在进入你的映射或 Storage 之前拒绝。

```typescript
import { room } from "./shared/messages"

const ADDRESS = /^0x[0-9a-f]{40}$/
const balances: Record<string, number> = {}

room.onMessage("giveCoins", (data, context) => {
  if (!context) return
  const from = context.from.toLowerCase()
  const to = String(data.to).toLowerCase()
  if (!ADDRESS.test(to) || to === from) return
  if (!Number.isInteger(data.amount) || data.amount <= 0) return // 拒绝负数和零
  if ((balances[from] ?? 0) < data.amount) return // 代币不足
  balances[from] -= data.amount
  balances[to] = (balances[to] ?? 0) + data.amount
})
```

### 在状态同步之前发送消息

客户端在交互之前必须等待状态同步完成：

```typescript
import { engine } from "@dcl/sdk/ecs"
import { isStateSyncronized } from "@dcl/sdk/network"

engine.addSystem(() => {
  if (!isStateSyncronized()) return
  // 可以安全发送消息
})
```

### 等待服务器启动

服务器只会在至少有一名玩家在场景中时保持活动状态。最后一名玩家离开后，服务器会继续运行大约两分钟，然后关闭。下一次访问会冷启动一个新的实例，这大约需要 **生产环境下 15 秒**。本地预览会立即启动服务器，因此冷启动问题几乎总是在发布之后才会暴露出来。

你的场景代码应准备好等待服务器上线。在服务器完成启动之前发送的消息会静默丢失。在等待时向玩家显示“服务器正在唤醒”提示，并为最初的服务器请求添加重试逻辑。


> [!NOTE]
> **提示：** 检测服务器就绪状态的一种可靠方法是心跳：让服务器每 2 秒向某个同步组件字段写入 `Date.now()` ，并让客户端跟踪自己上次看到该值变化的时间。如果 6 秒内没有任何变化，就把服务器视为离线。这比 `isStateSyncronized()`更稳健，因为后者只能确认传输已连接，并不能确认服务器正在运行。


## 完整示例

一个最小化的多人计数器：点击按钮，服务器会递增一个同步分数。服务器会将计数器持久化到 `Storage` ，这样该值就能在服务器重启后保留。记住，当场景中没有玩家时服务器会关闭，因此如果没有存储，每次场景变为空场时计数都会重置为零。

```typescript
import { engine, Schemas } from "@dcl/sdk/ecs"
import { registerMessages, isServer, syncEntity } from "@dcl/sdk/network"
import { AUTH_SERVER_PEER_ID } from "@dcl/sdk/network/message-bus-sync"
import { pointerEventsSystem } from "@dcl/sdk/ecs"
import { Storage } from "@dcl/sdk/server"

// 1. 定义消息（共享）
const Messages = {
  increment: Schemas.Map({}),
  stateUpdate: Schemas.Map({
    count: Schemas.Int,
    lastPlayer: Schemas.String,
  }),
}

// 2. 定义一个仅服务端组件（共享）
const Counter = engine.defineComponent("Counter", {
  value: Schemas.Int,
  lastPlayer: Schemas.String,
})

// 3. 创建房间
const room = registerMessages(Messages)

export async function main() {
  if (isServer()) {
    // === 服务器 ===

    // 只有服务器可以修改这个组件
    Counter.validateBeforeChange(
      (v) => v.senderAddress === AUTH_SERVER_PEER_ID
    )

    // 从存储中恢复计数器，以防服务器重启
    let initialCount = 0
    let initialPlayer = "none"
    let loaded = false
    try {
      initialCount = (await Storage.get<number>("counter")) ?? 0
      initialPlayer = (await Storage.get<string>("lastPlayer")) ?? "none"
      loaded = true
    } catch (error) {
      console.log("[服务器] 无法加载计数器：", error)
    }

    const counterEntity = engine.addEntity()
    syncEntity(counterEntity, [Counter.componentId], 1)
    Counter.create(counterEntity, {
      value: initialCount,
      lastPlayer: initialPlayer,
    })

    room.onMessage("increment", async (_data, context) => {
      if (!context) return
      if (!loaded) return // 绝不要覆盖一个未加载成功的计数器
      const from = context.from.toLowerCase()
      const counter = Counter.getMutable(counterEntity)
      counter.value += 1
      counter.lastPlayer = from

      // 持久化到存储，以便该值在服务器重启后仍能保留
      const saved =
        (await Storage.set("counter", counter.value)) &&
        (await Storage.set("lastPlayer", from))
      if (!saved) console.log("[服务器] 计数器未保存，下一次点击会再次保存它")

      room.send("stateUpdate", {
        count: counter.value,
        lastPlayer: from,
      })
    })
  } else {
    // === 客户端 ===
    const button = engine.addEntity()
    // ... 添加 Transform、MeshRenderer 等

    pointerEventsSystem.onPointerDown(button, () => {
      room.send("increment", {})
    })

    room.onMessage("stateUpdate", (data) => {
      console.log(`计数：${data.count}（最后一次点击来自 ${data.lastPlayer}）`)
    })
  }
}
```

## 本地测试

标准预览会处理所有事情。使用 SDK 的 auth-server 分支时，本地服务器会在后台与客户端预览一起自动启动。

要在本地测试多人交互，请在两个独立窗口中打开预览，每个窗口都被视为一个独立玩家。用不同的地址连接每个窗口。两个客户端都会连接到同一个本地服务器实例。

在 Creator Hub 中，开启 **多实例预览** ，位于 **桌面客户端** 的 **播放选项**子菜单中，然后第二次点击 **播放** 按钮，以打开第二个 Decentraland 浏览器窗口。你必须在两个窗口中使用不同地址连接。随着场景重新加载，相同会话会保持打开。

或者，你也可以通过在浏览器 URL 中输入以下内容来打开第二个 Decentraland 浏览器窗口：

> `decentraland://realm=http://127.0.0.1:8000&local-scene=true&debug=true&multi-instance=true`

### 调试技巧

* *给你的日志加上前缀* 使用 `[SERVER]` 或 `[CLIENT]` ，这样你就能在终端中区分它们：

  ```typescript
  import { isServer } from "@dcl/sdk/network"

  if (isServer()) {
    console.log("[SERVER] 启动中...")
  } else {
    console.log("[CLIENT] 启动中...")
  }
  ```
* *验证组件同步* ，通过记录实体数量来在客户端检查：

  ```typescript
  import { engine } from "@dcl/sdk/ecs"

  engine.addSystem(() => {
    const entities = Array.from(engine.getEntitiesWith(MyComponent))
    console.log("[CLIENT] 已同步实体：", entities.length)
  })
  ```

## 在生产环境中调试

使用 [场景洞察](/docs/creator/content-creator-zh/chang-jing-sdk7/tiao-shi/scene-insights/) 以搜索已发布的服务器日志、实时跟踪它们，并获取 CPU、内存和其他指标。日志和指标会保留 **7 天**，跨越重启和重新部署。

更新到最新的 `auth-server` SDK，然后在你的场景项目中运行这些命令：

```bash
npx sdk-commands scene-insights login
npx sdk-commands scene-insights tail
```

在浏览器中使用已授权的钱包登录。此只读会话会被复用于后续命令，最长可达 12 小时。按下 **Ctrl+C** 可停止跟踪日志。

该 [Scene Insights 指南](/docs/creator/content-creator-zh/chang-jing-sdk7/tiao-shi/scene-insights/) 涵盖历史搜索、场景选择、访问权限、指标、AI 辅助调试以及采集限制。

### 查看存储数据

你可以通过输入这个链接来访问场景存储中保存的数据：

[decentraland.org/storage](https://decentraland.org/storage)

你也可以通过 Creator Hub 打开此页面。打开 **管理** 选项卡，点击你已发布内容的位置旁边的三个点，然后选择 **查看存储**.

在那里，你可以看到所有可以发布场景的世界和土地列表。

打开世界或玩家数据，查看为每个对象存储的信息。

例如，如果某个特定玩家在游玩你的场景时遇到问题，你可以通过地址查找这个玩家，并查看为他们存储了哪些数据，以了解他们的情况。也许他们碰到了一个边缘情况，导致数据彼此矛盾。你甚至可以在这个页面上清除或编辑该玩家的数据，将其恢复到稳定状态。

## 版本控制

每个已发布的场景版本都会获得自己独特的哈希 ID，并且每个哈希都对应自己的服务器实例。这意味着客户端代码和服务器代码始终同步演进，不会出现客户端运行旧逻辑却连接到运行新逻辑的服务器（反之亦然）的时间窗口。

当你发布更新时：

* *已经在场景中的玩家* 会继续看到旧版本的场景，直到他们离开并回来。他们的客户端会一直连接到与旧哈希匹配的服务器实例。
* *新加入的玩家* 在更新后会加载新版本的场景并连接到新的服务器实例。

这确保了由于架构变更或组件重命名，客户端和服务器状态永远不会失去同步。更新绝不会破坏已经在你场景中的玩家的会话。

代价是，在部署后的很短一段时间内，玩家可能会被分散到两个不同的服务器实例上。已经在场景中的玩家和刚加入的玩家可能彼此看不到，也无法通过场景进行交互，即使他们在同一个场景中，也要等到较早的玩家离开并重新加入后才行。


> [!NOTE]
> **💡 提示**：通过 [Storage](#data-storage) 服务（如排行榜、玩家进度或持久化的环境更改）存储的数据会 *未* 在不同版本之间被清除。存储是在场景级别持久化的，并在指向同一场景的所有服务器实例之间共享，因此新版本会从上一个版本停下的地方继续。


## 从 Colyseus 迁移

如果你有一个基于 Colyseus 构建的现有场景，下表会将常见的 Colyseus 模式映射到其对应的 SDK7 实现：

| Colyseus                      | SDK7 多人游戏服务器                         |
| ----------------------------- | ------------------------------------ |
| `room.send(type, data)`       | `room.send(type, data)` — 相同的 API    |
| `room.onMessage(type, cb)`    | `room.onMessage(type, cb)` — 相同的 API |
| `room.state.players` （schema） | `syncEntity` + 自定义组件                 |
| JSON 序列化                      | 二进制序列化（通过……自动 `Schemas`)             |
| 独立的服务器应用程序                    | 同一代码库—— `isServer()` 分支              |
| 自定义服务器托管                      | 内置：预览会自动运行服务器                        |

需要注意的关键差异：

* *序列化*：Colyseus 发送 JSON 差异；SDK 在每次更改时都会发送整个组件。请保持组件尽可能小（参见 [性能最佳实践](#performance-best-practices)).
* *状态模型*：Colyseus 使用带自动差异同步的可变状态树。SDK 使用通过……同步的 ECS 组件 `syncEntity` 并通过 `validateBeforeChange`.
* *托管*：无需单独部署服务器。多人服务器会随着场景一起自动部署。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/lian-wang/authoritative-servers.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
