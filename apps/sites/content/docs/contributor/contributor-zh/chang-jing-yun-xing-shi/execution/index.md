# 执行

运行时将 [场景](/docs/contributor/contributor-zh/nei-rong/shi-ti-lei-xing/scenes/) 像典型的 Node 模块一样。一旦沙箱设置完成（即所有 [全局变量](/docs/contributor/contributor-zh/chang-jing-yun-xing-shi/globals/) 已注入且 [模块](https://github.com/decentraland/docs/tree/main/contributor/runtime/modules/README.md) 已准备好加载），场景的代码将被求值，以便填充 [exports](/docs/contributor/contributor-zh/chang-jing-yun-xing-shi/globals/#module) 对象。

主机将从 `exports`: [`onStart`](#onstart) （可选）和 [`onUpdate`](#onupdate) （必需）中获取两个方法。

```ts
类型 Exports = {
  onStart?: () => Promise<void>;
  onUpdate: (deltaSeconds: number) => Promise<void>;
};
```

在场景的生命周期内，运行时将确保对这些方法的调用绝不会并发进行。返回的 `Promise` 在场景收到新的调用之前，都会始终等待其完成。

```mermaid
流程图 LR
    A[onStart] -->|await| B[onUpdate]
    B -->|await| B
```


> [!NOTE]
> 场景与游戏渲染引擎之间高效且性能良好的同步是一个复杂的问题。你可以在基金会的实现中找到相关经验，参见 [ADR-148](https://adr.decentraland.org/adr/ADR-148).


#### `onStart` <a href="#onstart" id="onstart"></a>

场景的生命周期始于异步的 `onStart` 方法。这是进行一次性初始化的地方，这些初始化必须在首次调用 [`onUpdate`](#onupdate) 之前完成。

```ts
onStart(): Promise<void>
```

场景应使用此调用从运行时请求任何已有状态（例如 [基础实体](/docs/contributor/contributor-zh/chang-jing-yun-xing-shi/basic-entities/)）并执行它们自己的初始设置。

导出 `onStart` 建议所有场景都导出（在使用 SDK 时会自动完成），但协议并不强制要求。该方法可能缺失，或者返回一个立即已解决的 `Promise`.

#### `onUpdate` <a href="#onupdate" id="onupdate"></a>

当场景处于主动运行状态时，异步的 `onUpdate` 方法将由运行时定期调用，以报告时间的流逝。这是场景的核心：在每次后续调用中，都可以更新实体、处理输入、播放动画、发送消息、显示 UI 等。

```ts
onUpdate(deltaSeconds: number): Promise<void>
```

该 `deltaSeconds` 参数是自上次调用以来经过的秒数的小数部分 `onUpdate` 是由运行时发起的（不论返回的 `Promise` 何时结束）。这对每个正在运行的场景而言都是特定的。

由于 `onUpdate` 调用绝不会重叠， `deltaSeconds` 的顺序会形成连贯的时间线。

在理想情况下，场景有机会运行 `onUpdate` 在每一帧渲染之前。然而，取决于可用资源，负载较重的场景可能无法及时完成其 `Promise` 以赶上下一帧。就这一点而言，每个场景都是独立的：轻量级实现的 `onUpdate` 可能会在同一时间段内被调用多次，而另一个场景中的一次耗时调用仍在进行。

参见 [ADR-148](https://adr.decentraland.org/adr/ADR-148) 了解详情。

### 运行多个场景

World Explorer 不仅必须支持运行多个沙箱化场景，而且应始终如此。这样做有两个原因：

首先，当玩家探索世界时，他们的视野会超出其所站立场景的地理边界。附近的场景（距离可为任意值或可配置）必须运行，才能生成玩家应当能够看到的内容。

其次，场景并不只是运行在各自互不重叠的边界内。它们也可以相互叠加，影响玩家周围的区域，同时提供不同的功能。

这一点尤其适用于两类场景：头像场景和可携带体验。

#### 头像场景 <a href="#avatarscene" id="avatarscene"></a>

从玩家进入世界的那一刻起，一个负责渲染其他玩家头像的全局场景就开始运行。

这个场景不受地理边界限制，无论玩家身处何处，都可以显示其中的实体（以及用于通信的 UI 小部件）。

#### 可携带体验

Decentraland 允许可穿戴物品拥有自己的行为，该行为在其自己的场景上下文中运行。这些场景以玩家为中心，并可通过添加实体或显示 UI 小部件来丰富体验。

当可穿戴物品被装备时，它们开始运行；当被移除时停止运行。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-zh/chang-jing-yun-xing-shi/execution.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
