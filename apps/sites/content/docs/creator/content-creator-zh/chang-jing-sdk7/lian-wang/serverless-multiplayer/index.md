> 在玩家之间同步场景状态。

# 无服务器多人模式

Decentraland 在玩家的 explorer 实例中本地运行场景。默认情况下，玩家可以彼此看见并直接交互，但每个玩家都是独立与环境交互。默认情况下，环境中的变化不会在玩家之间共享。

让玩家在相同状态下看到相同内容，对于以更有意义的方式进行交互非常重要。

有三种方式可以同步场景状态，使所有玩家看到相同的内容：

* **将实体标记为已同步**：最简单的选项。见 [将实体标记为已同步](#mark-an-entity-as-synced)
* **发送显式的 MessageBus 消息**：手动发送并监听特定消息。见 [发送显式的 MessageBus 消息](#send-explicit-messagebus-messages)
* **使用多人服务器**：见 [多人服务器](/docs/creator/content-creator-zh/chang-jing-sdk7/lian-wang/authoritative-servers/)。服务器验证所有状态变化，并且是唯一的事实来源。需要更多配置，但如果玩家有动机利用你的场景，强烈建议这样做。

本文档介绍了前两种选项。它们更简单，因为不需要服务器。缺点是你更依赖玩家的连接速度，并且当所有玩家离开场景时，场景状态不会被持久保存。

## 将实体标记为已同步

在 [Creator Hub](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/ru-men/about-editor/)中，通过添加 **多人组件** 将一个实体标记为已同步。它包含该实体上其他每个组件的复选框，允许你选择要更新哪些组件。

![](/docs/_remote/710c6ed40b2f7a539e355f1b.webp)

要通过代码将实体标记为已同步，请使用 `syncEntity` 函数：

```ts
import { syncEntity } from "@dcl/sdk/network";

export function main() {
  const doorEntity = engine.addEntity();

  syncEntity(doorEntity, [Transform.componentId, Animator.componentId], 1);
}
```


> [!WARNING]
> **📔 注意**：务必在 `syncEntity()` 内部调用 `main()` 函数，或者在一个会在 `main()` 之后运行的函数中调用它（例如回调或系统）。在文件顶层调用会抛出错误，因为此时玩家配置文件尚未初始化。



> [!WARNING]
> **📔 注意**：在无服务器多人模式中，每个客户端都会自行调用 `syncEntity` 。如果你升级到 [多人服务器](/docs/creator/content-creator-zh/chang-jing-sdk7/lian-wang/authoritative-servers/)，模式会改变：只有服务器应该调用 `syncEntity`，并由 `isServer()`进行保护。客户端应避免声明共享实体的同步，除非它们是由客户端创建的。


该 `syncEntity` 函数接受以下输入：

* **entityId**：要同步的实体引用
* **componentIds**：需要从该实体同步的组件列表。这是一个数组，可以包含所需数量的组件。所有值都应为 `componentId` 属性。
* **entityEnumId**：（可选）一个由所有玩家一致使用的唯一 ID，见 [关于枚举 ID](#about-the-enum-id).

并非所有实体或组件都需要同步。像始终停留在同一位置的树这样的静态元素不需要同步。在你确实进行同步的实体上，只有随时间变化的组件才应该被同步。例如，如果一个立方体在被点击时改变颜色，你只应该同步 Material 组件，而不需要同步 MeshRenderer 或 Transform，因为它们不会改变。


> [!NOTE]
> **💡 提示**：如果你想共享的数据并不存在于某个组件中，请定义一个 [自定义组件](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/custom-components/) 来保存这些数据。


### 实体的枚举 ID

该 **entityEnumId** 必须唯一。它与 `engine.addEntity()`上分配的本地 entityId 无关；后者会自动生成，并且在运行同一场景的不同玩家之间可能不同。实体的 entityEnumId 必须在代码中显式定义，并且必须唯一。

显式设置此 ID 很重要，可避免因竞态条件导致场景一部分比另一部分更早加载而出现不一致。也许对玩家 A 来说，场景中的门是实体 *512*，但对玩家 B 来说，同一扇门是实体 *513*。在这种情况下，如果玩家 A 打开门，玩家 B 看到的却是整栋建筑在移动。


> [!NOTE]
> **💡 提示**：在你的场景中创建一个 enum，以便清晰地引用场景中每个可同步的 ID。
> 
> ```ts
> import { syncEntity } from "@dcl/sdk/network";
> 
> enum EntityEnumId {
>   DOOR = 1,
>   DRAW_BRIDGE = 2,
>   ELEVATOR = 3,
> }
> 
> export function main() {
>   syncEntity(
>     doorEntity,
>     [Transform.componentId, Animator.componentId],
>     EntityEnumId.DOOR
>   );
> }
> ```
> 
> 这里使用 EntityEnumId enum 为实体标记唯一标识符，确保每个客户端都能识别被修改的实体，而不受创建顺序影响。



> [!WARNING]
> **📔 注意**：如果你的场景还包含 Smart Items，请避免使用高于 **8001** 的数字。由 [Creator Hub](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/ru-men/about-editor/) 带有 Multiplayer 组件创建的物品会自动使用从 8001 开始的 ID。任何低于 8001 的 ID 都可以安全分配给你的已同步实体。


**由玩家创建的实体**

如果一个实体是由玩家交互产生的，并且这个实体应该与其他玩家同步，那么它不需要 entityEnumId。你可以使用 `syncEntity()` ，只传入实体和组件列表即可。entityEnumId 的唯一值会在幕后自动分配。

在场景初始化时实例化的所有实体都需要手动分配 ID。这样可以确保所有玩家在每个实例中都使用相同的 ID。当单个玩家负责实例化某个实体时，就不需要显式 ID。其他玩家会收到这个新实体的更新，其 ID 已经分配好，因此不会有 ID 不匹配的风险。

例如，在一个打雪仗场景中，每次玩家扔出一个雪球时，都会实例化一个新的实体，并与其他玩家同步。这个雪球不需要唯一的 entityEnumId。

```ts
import { syncEntity } from "@dcl/sdk/network";

function onThrow() {
  const ball = engine.addEntity();
  Transform.create(ball, {});
  GltfContainer.create(ball, { src: "assets/snowBall.glb" });
  syncEntity(ball, [Transform.componentId, GltfContainer.componentId]);
}
```

### 父子实体 <a href="#parented-entities" id="parented-entities"></a>

实体的父级通常通过 `parent` 组件中的 `变换` 属性定义。不过，这个属性指向的是父级的本地实体 ID，而它可能会不同，见 [关于枚举 ID](#about-the-enum-id)。对于需要同步的实体，或者其子实体需要同步的实体，请使用 `parentEntity()` 函数，而不是 `变换`.

```ts
import { syncEntity, parentEntity } from "@dcl/sdk/network";

export function main() {
  const parent = engine.addEntity();
  Transform.create(parent, { position: somePosition });
  syncEntity(parent, []);

  const child: Entity = engine.addEntity();
  syncEntity(child, [Transform.componentId]);

  parentEntity(child, parent);
}
```

请注意，父实体和子实体都通过 `syncEntity`进行了同步，因此所有玩家都能对这两个实体使用的 ID 达成一致。即使父实体的组件可能永远不需要改变，这也是必要的。在此示例中， `syncEntity` 包含了一个空的组件数组，以避免同步任何不必要的组件。


> [!WARNING]
> **📔 注意**：如果一个实体同时由 `parentEntity()` 以及 `parent` 组件中的 `变换` 组件建立父子关系，则 `变换` 组件中的属性会被忽略。


当实体通过 `parentEntity()` 函数建立父子关系时，你还可以使用以下辅助函数：

* **removeParent()**：撤销 `parentEntity()`的效果。它只需要你传入子实体。该实体的新父级将成为场景的根实体。原始父实体不会从场景中移除。
* **getParent()**：返回你传入的实体的父实体。
* **getChildren()**：返回你传入实体的子实体列表，作为可迭代对象。
* **getFirstChild()**：返回你传入实体列表中的第一个子实体。

```ts
import {
  syncEntity,
  parentEntity,
  getParent,
  getFirstChild,
  getChildren,
  removeParent,
} from "@dcl/sdk/network";

export function main() {
  const parent = engine.addEntity();
  Transform.create(parent, { position: somePosition });
  syncEntity(parent, []);

  const child: Entity = engine.addEntity();
  syncEntity(child, [Transform.componentId]);

  // 将 parent 设置为父级
  parentEntity(child, parent);

  // getParent
  const getParentResult = getParent(child);
  // 返回父级

  // getFirstChild
  const getFirstChildResult = getFirstChild(parent);
  // 返回子级

  // getChildren
  const getChildrenResult = Array.from(getChildren(parent));
  // 返回 [child]

  // 从 child 中移除父级
  removeParent(child);
}
```

## 检查同步状态

当玩家刚进入场景时，他们可能尚未与周围其他玩家同步。如果玩家在同步之前开始更改游戏状态，这可能会导致你的游戏出现问题。我们建议始终在允许玩家编辑场景中的任何内容之前，先检查玩家是否已同步。

如果玩家走出场景所在的地块，他们在站到外面时也会与场景不同步。因此，场景的系统也需要处理这种情况，因为即使玩家离开，场景仍会继续运行。一旦玩家重新进入，他们会自动收到场景状态中任何变化的更新。

你可以通过 `isStateSyncronized()` 函数检查场景状态当前是否已为某个玩家同步。此函数返回一个布尔值，如果该玩家已经与场景同步，则为 true。

```ts
import { isStateSyncronized } from "@dcl/sdk/network";

const isConnected = isStateSyncronized();
```

例如，你可以在系统中加入此检查，并在该函数返回 false 时阻止任何交互。

```ts
import { isStateSyncronized } from "@dcl/sdk/network";

engine.addSystem(() => {
  if (isStateSyncronized() && !button.enabled) {
    console.log("启用开始游戏");
    button.enable();
  }

  if (!isStateSyncronized() && button.enabled) {
    console.log(`禁用开始游戏。`);
    button.disable();
  }
});
```

## 发送显式的 MessageBus 消息


> [!WARNING]
> **📔 注意**： `MessageBus` API 在 SDK 中被标记为已弃用，未来版本中可能会被移除。对于大多数用例，建议优先 [将实体标记为已同步](#mark-an-entity-as-synced).


**初始化消息总线**

创建一个消息总线对象，用于处理在玩家之间发送和接收消息所需的方法。

```ts
import { MessageBus } from "@dcl/sdk/message-bus";

const sceneMessageBus = new MessageBus();
```

**发送消息**

使用消息总线的 `.emit` 命令向场景中的所有其他玩家发送消息。

```ts
import { MessageBus } from "@dcl/sdk/message-bus";

const sceneMessageBus = new MessageBus();

const myEntity = engine.addEntity();
MeshRenderer.setBox(myEntity);
MeshCollider.setBox(myEntity);

pointerEventsSystem.onPointerDown(
  {
    entity: myEntity,
    opts: { button: InputAction.IA_PRIMARY, hoverText: "点击" },
  },
  function () {
    sceneMessageBus.emit("box1Clicked", {});
  }
);
```

每条消息都可以在第二个参数中包含一个有效载荷。该有效载荷的类型为 `Object`，可以包含你希望发送的任何相关数据。

```ts
import { MessageBus } from "@dcl/sdk/message-bus";

const sceneMessageBus = new MessageBus();

sceneMessageBus.emit("spawn", { position: { x: 10, y: 2, z: 10 } });
```


> [!NOTE]
> **💡 提示**：如果你需要一条消息包含多个变量的数据，请创建一个自定义类型，将所有这些数据保存在一个对象中。


**接收消息**

要处理该场景中所有其他玩家发来的消息，请使用 `.on`。使用此函数时，你需要提供一个消息字符串，并定义一个要执行的函数。每当有匹配字符串的消息到达时，给定的函数就会执行一次。

```ts
import { MessageBus } from "@dcl/sdk/message-bus";

const sceneMessageBus = new MessageBus();

type NewBoxPosition = {
  position: { x: number; y: number; z: number };
};

sceneMessageBus.on("spawn", (info: NewBoxPosition) => {
  const myEntity = engine.addEntity();
  Transform.create(myEntity, {
    position: { x: info.position.x, y: info.position.y, z: info.position.z },
  });
  MeshRenderer.setBox(myEntity);
  MeshCollider.setBox(myEntity);
});
```


> [!WARNING]
> **📔 注意**：玩家发送的消息也会被该玩家自己接收到。 `.on` 方法无法区分消息是由该玩家自己发出的，还是由其他玩家发出的。


**完整的 MessageBus 示例**

此示例使用消息总线，在每次点击主立方体时发送一条新消息，并在随机位置生成一个新立方体。消息包含新立方体的位置，因此所有玩家都能在相同的位置看到这些新立方体。

```ts
import { MessageBus } from "@dcl/sdk/message-bus";

/// --- 创建消息总线 ---
const sceneMessageBus = new MessageBus();

// 立方体工厂
function createCube(x: number, y: number, z: number): Entity {
  const meshEntity = engine.addEntity();
  Transform.create(meshEntity, { position: { x, y, z } });
  MeshRenderer.setBox(meshEntity);
  MeshCollider.setBox(meshEntity);

  // 当立方体被点击时，发送消息生成另一个立方体
  pointerEventsSystem.onPointerDown(
    {
      entity: meshEntity,
      opts: { button: InputAction.IA_PRIMARY, hoverText: "按 E 生成" },
    },
    function () {
      sceneMessageBus.emit("spawn", {
        position: {
          x: 1 + Math.random() * 8,
          y: Math.random() * 8,
          z: 1 + Math.random() * 8,
        },
      });
    }
  );

  return meshEntity;
}

// 初始化
createCube(8, 1, 8);

// 定义数据类型
type NewBoxPosition = {
  position: { x: number; y: number; z: number };
};

// 在 spawn 消息到达时，创建新的立方体
sceneMessageBus.on("spawn", (info: NewBoxPosition) => {
  createCube(info.position.x, info.position.y, info.position.z);
});
```

## 在本地测试多人场景

如果你启动场景预览并在两个（或更多）不同的 Explorer 窗口中打开它，每个打开的窗口都会被视为独立玩家，模拟通信服务器会让这些玩家保持同步。

在一个窗口中与场景互动，然后切换到另一个窗口，以查看该互动的效果是否也在其中可见。

使用 Creator Hub，开启 **多实例预览** 在 **桌面客户端** 的子菜单 **播放选项**，然后再次点击 **播放** 按钮，以打开第二个 Decentraland Explorer 窗口。你必须在两个窗口中使用不同的地址连接。场景重新加载时，这些会话将保持打开。

![](/docs/_remote/ca7e6550cdb1bb947ce74b28.webp)

或者，你可以通过在浏览器 URL 中输入以下内容来打开第二个 Decentraland Explorer 窗口：

> `decentraland://realm=http://127.0.0.1:8000&local-scene=true&debug=true&multi-instance=true`

## 单人场景

如果你的场景部署到 [Decentraland World](/docs/creator/content-creator-zh/chang-jing-sdk7/fa-bu/publishing-options/#decentraland-worlds)，你可以将其设为单人场景。玩家彼此看不到，无法聊天，也看不到彼此行为的影响。

要做到这一点，请配置场景的 `scene.json` 文件，将 **fixedAdapter** 设置为 `offline:offline`。该场景将完全没有通信服务，加入该 world 的每个用户都将始终独自一人。

**示例：**

```json
{
  "worldConfiguration": {
    "name": "my-name.dcl.eth",
    "fixedAdapter": "offline:offline"
  }
}
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/lian-wang/serverless-multiplayer.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
