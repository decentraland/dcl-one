> 了解如何处理场景中的用户点击。

# 高级按钮事件

如果你需要场景中的交互遵循与 [注册回调](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/register-callback/) 或 [基于系统的](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/system-based-events/) 方法不兼容的自定义逻辑，你可以直接处理原始输入事件数据。这种方法最难，但也最灵活。

## PointerEventsResult 组件

当引擎检测到指针事件时，被点击的实体会被分配一个 `PointerEventsResult` 组件。该组件包含有关命中事件的所有原始数据，并存储之前事件的历史数据。

中的辅助函数 `inputSystem`一样的样式属性，例如 `inputSystem.isTriggered` 或 `inputSystem.getInputCommand` 适用于大多数简单场景，但如果你需要获取有关命中事件的更多细节，请查看 `PointerEventsResult`.

该 `PointerEventsResult` 组件保存一个指针事件列表，每个事件对应一个对象。它最多存储 100 个事件，较新的事件存储在列表末尾。当列表长度达到 100 时，每当有新事件进入，它就会开始丢弃旧事件。

列表中的每个事件都包含以下数据：

* `模拟`：可选数字，仅在来自模拟输入（如摇杆）的事件中出现，存储输入的模拟值。
* `按钮将你的钱包连接起来`：按下了哪个按钮 ID。该数字对应于 `InputAction` 枚举，其中列出了所有可用按钮。
* `state`：指针事件类型，来自枚举 `PointerEventType`. *0* 指的是 `PointerEventType.PET_UP`, *1* 设置为 `PointerEventType.PET_DOWN`, *2* 设置为 `PointerEventType.PET_HOVER_ENTER`, *3* 设置为 `PointerEventType.PET_HOVER_LEAVE`, *4* 设置为 `PointerEventType.PET_PROXIMITY_ENTER`, *5* 设置为 `PointerEventType.PET_PROXIMITY_LEAVE`
* `timestamp`: 一个 [Lamport 时间戳](https://en.wikipedia.org/wiki/Lamport_timestamp) 用于标识每个按钮事件。

  > 注意：此时间戳并不是根据当前时间编号的。可以把它看作一个从 0 开始的计数器，每发生一个事件就加 1。
* `命中`：一个对象，包含以下有关命中事件的数据：
  * `entityId`：被射线击中的实体的 ID 编号。
  * `meshName`: *字符串* ，其中包含被击中的 3D 模型中特定网格的内部名称。当 3D 模型由多个网格组成时，这很有用。
  * `globalOrigin`: *Vector3* 表示射线起点的位置（相对于场景）
  * `direction`: *Vector3* 以及射线的方向向量，采用全局坐标
  * `position`: *Vector3* 表示射线与命中的实体相交的位置（相对于场景）
  * `length`：射线从起点到与实体发生命中的位置之间的长度。
  * `normalHit`: *Vector3* ，使用归一化方向向量，描述世界空间中命中法线的角度。

```ts
import { engine, PointerEventsResult } from '@dcl/sdk/ecs'

function PointerReadingSystem() {
  for (const [entity] of engine.getEntitiesWith(PointerEventsResult)) {
    const events = PointerEventsResult.get(entity)
    for (const event of events) {
      console.log('指针事件数据：', event)
    }
  }
}

engine.addSystem(PointerReadingSystem)
```

## 跟踪玩家移动

在实时多人游戏中，如果玩家移动的时机至关重要，你可能希望使用第三方服务器作为事实来源来跟踪每个玩家的位置。你可以提前监听按钮，并在头像移动之前在服务器端预测它们的效果，从而提高响应时间。

这种方法有助于弥补网络延迟，但肯定会导致偏差，因此你也应该定期轮询玩家当前的位置以进行修正。平衡这些预测和修正可能需要大量调优。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/advanced-button-events.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
