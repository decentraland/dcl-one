# 引擎 API

该 `EngineApi` 模块提供对实体-组件-系统（Entity-Component-System）框架的访问，该框架由世界浏览器（运行游戏循环）和各个场景（管理各自实体）共享，同时还包括相关工具。

```ts
const engine = require("~system/EngineApi");
```

这个模块功能最复杂、最丰富，也最有可能收到更新和扩展。Decentraland 的大部分能力都在这里，因为它决定了人们可以创建何种体验。

这个模块包含以下方法：

* [`函数 crdtSendToRenderer`](#crdtsendtorenderer)
* [`函数 crdtGetState`](#crdtgetstate)

## 简介

该 `EngineApi` 模块旨在通过交换更新、事件和命令，在 Explorer 与场景之间同步世界状态。它实现了一种可扩展的消息协议，使场景能够，除其他外：

* 创建和销毁实体。
* 附加、更新和移除组件。
* 在 3D 环境中发射射线并检测命中。
* 接收诸如来自玩家的输入之类的事件。

```
.------------------------------------------------------.
| 世界浏览器                                       |
|                                                      |
|                   [引擎 API]                       |
|                        |                             |
|  .--------.            |              .------------. |
|  |        |<-----------+<-------------+  运行时   | |
|  |  游戏  |            |   命令      |  .------.  | |
|  |        |   事件    |              |  | 场景 | | |
|  | 引擎 +----------->+------------->|  |       | | |
|  |        |            |              |  '-------' | |
|  '--------'            |              '------------' |
|                                                      |
'------------------------------------------------------'

```


> [!NOTE]
> 该 `EngineApi` 模块正在重构。如果你查看源定义，会发现一些旧方法在最新版本中已经不再使用，或者可以实现为空操作（下面会进一步说明）。


## ECS 框架

借助 `EngineApi` 模块，提供了一个通用且可扩展的共享 ECS 框架实现，使场景和游戏引擎本身都能创建实体、附加组件并更新它们的状态。

任一方所做的更改都会通过交换消息反映到另一方。所使用的机制（[见下文](#synchronization)）确保双方就任何更新的顺序达成一致，并在世界状态上最终达到一致。

世界浏览器实现了一套众所周知的基础组件（位置、形状、纹理、媒体等），并且知道如何在收到更新时反序列化并应用其状态变化。使用 [SDK](https://docs.decentraland.org/creator) 的场景也有工具来创建自定义组件，但这些完全由场景管理（不进行同步），因此不在该协议的范围内。


> [!NOTE]
> 大多数场景使用 Decentraland SDK，它封装了 `EngineApi` 模块，并为内容开发者提供了更友好、更高层的接口。直接访问本模块中消息协议的场景极为罕见。


### 实体标识 <a href="#identifying" id="identifying"></a>

实体 ID 是从 `0`.

根据协议，小于 `512` 的 ID 保留给世界浏览器，因此对场景创建的实体无效。数字 `0`, `1` 和 `2` 实际上正在使用中（见 [基础实体](/docs/contributor/contributor-zh/chang-jing-yun-xing-shi/basic-entities/)），其余范围可供未来扩展。

每个场景都有自己私有的 ID 范围，Explorer 可以透明地将其映射到游戏引擎中所有实体的全局标识符。

随着时间推移，当实体被创建和删除时，重用之前已释放的 ID 是完全合理的——实际上，这可能出于性能原因而是必须的（[见下文](#crdtstatedeleteentities)).

## 同步 <a href="#synchronization" id="synchronization"></a>

世界浏览器和场景都必须管理一组共享实体和组件，其中更新会以异步方式来自双方。若没有额外措施，它们对世界状态的版本最终会（而且会）不同。

为防止这种情况，使用 CRDT（[冲突自由复制数据类型](https://en.wikipedia.org/wiki/Conflict-free_replicated_data_type)）通过交换带更新的消息来让双方达成一致。这带来若干优势：

* 双方可以独立且并发地更新状态。
* 冲突由双方本地应用的共享解决策略来处理。
* 保证双方最终收敛到共享且相同的状态。
* 除了消息大小有少量开销之外，不需要额外的协调或消息往返。


> [!NOTE]
> 本页解释了如何使用 CRDT 在本地运行的场景与游戏引擎之间同步状态，但这种机制的真正潜力在于多人场景。使用该协议，玩家可以协调自己的游戏状态并共享相同的体验。


要实现这一点，所有消息都必须满足可交换性和幂等性。乱序消息即使在某个被认为是中间更新的消息尚未收到时也可以应用，而相同的消息可以在不破坏一致性的情况下被重复处理。

让我们重新看一下上面的架构图，现在重点关注 CRDT：

```
.-------------------------------------------------------.
| 世界浏览器                                        |
|                                                       |
| .-------------.                       .------------.  |
| | 游戏引擎 |                       |   场景     | |
| |  .-------.  |                       |  .-------.  | |
| |  |       +--+-----------------------+->|       |  | |
| |  | CRDT  |  |     同步协议     |  | CRDT  |  | |
| |  |       |<-+-----------------------+--+       |  | |
| |  '-------'  |                       |  '-------'  | |
| |             |                       |             | |
| '-------------'                       '-------------' |
'-------------------------------------------------------'

```

CRDT 同步机制的实现由三部分组成：

1. 所有实体和组件状态的内部表示。
2. 用于在竞争状态之间进行选择的冲突解决策略。
3. 用于通信并处理状态更新的消息协议。

### CRDT 状态 <a href="#crdtstate" id="crdtstate"></a>

CRDT 保存着所有组件及其所附加实体的状态，并且可以存储为每个实体的一份组件到带时间戳状态的映射。伪代码如下：

```ts
// 某个特定实体在某一时刻的组件状态（完全通用）：
type EntityComponentState = { timestamp: number; state: byte[] };

// 一个组件对应所有已附加实体的状态：
type ComponentState = Map<EntityId, EntityComponentState>;

// CRDT 的完整状态：
type CRDTState = Map<ComponentId, ComponentState>;
```

时间戳实际上并不是 Unix 风格的时间戳。它们是一种用于对更新进行排序的递增计数器，与时钟时间没有关系。关于这一点的更多内容 [下文](#timestamps).

#### 创建状态条目 <a href="#crdtstateautocreate" id="crdtstateautocreate"></a>

由于 CRDT 层并不知道（也无需知道）所有可用的组件，初始状态是一个空映射。当操作涉及一个尚未映射的 `ComponentId` 或 `EntityId` 时，会动态创建条目。当一个实体被移除时，关联状态会从所有组件中删除。

让我们用更多伪代码来说明这一点。

```ts
const crdtState = new Map(); // 为空，我们会自动为新组件和实体预留空间
```

```ts
function putEntityComponentState(componentId, entityId, entityComponentState) {
  // 如果这个组件还未知，就把它加进去：
  if (!crdtState[componentId]) {
    crdtState[componentId] = new Map();
  }

  // 如果这个实体在这个组件上没有状态，就添加它并结束：
  if (!crdtState[componentId][entityId] && shouldCreate(entityId)) {
    crdtState[componentId][entityId] = entityComponentState;
    return;
  }

  // 到这里，我们有两个竞争状态。我们刚收到的那个不一定是
  // 应该保留的那个。我们需要做出决定。
  const existingEntityComponentState = crdtState[componentId][entityId];

  // 只有当冲突解决规则要求这样做时（这一点稍后会详细说明），才替换该状态：
  if (shouldReplace(entityComponentState, existingEntityComponentState)) {
    crdtState[componentId][entityId] = entityComponentState;
  }
}
```

上面的伪代码（如果转换为实际代码）显然是不完整且次优的。特别是，它缺少 `shouldCreate` 和 `shouldReplace`的定义，而它们封装了冲突解决策略。这些都有要求：

* `shouldCreate` 必须添加新实体，但拒绝重新创建已删除的实体。
* `shouldReplace` 必须优先保留时间戳更大的状态（某些边缘情况除外）。

#### 跟踪已删除的实体 <a href="#crdtstatedeleteentities" id="crdtstatedeleteentities"></a>

为支持 `shouldCreate` 这一要求，你需要跟踪已删除的实体 ID，以便忽略之后针对这些实体的任何状态更新。让我们用更多伪代码来看一下：

```ts
const crdtDeletedEntities = new Set();

function shouldCreate(entityId) {
  return !crdtDeletedEntities.contains(entityId);
}

function deleteEntity(entityId) {
  // 将这个实体标记为已删除：
  crdtDeletedEntities.add(entityId);

  // 删除每个组件中该实体的状态：
  for (componentState of crdtState) {
    delete componentState[entityId];
  }
}
```


> [!NOTE]
> 请注意，在上面这个朴素实现中，已删除实体的集合只会不断增长。在快速循环复用实体的场景中，这可能导致内存使用量激增。
> 
> 为避免这种情况，Foundation 的 Explorer 使用世代索引在有限的数值空间中重用 ID，从而限制跟踪活动实体和已删除实体所需的内存量。


至于 `shouldReplace` 这一要求，请见 [冲突解决](#crdtconflicts) 下文。

### 时间戳 <a href="#timestamps" id="timestamps"></a>

如上所述，当谈到 CRDT 时间戳时，我们并不是指某一时刻的实际 Unix 时间戳。相反，使用的是 [Lamport 时间戳](https://en.wikipedia.org/wiki/Lamport_timestamp) （一种特殊的共享递增计数器）来跟踪双方事件的顺序。

该计数器的值根据以下规则更新：

1. 将本地计数器初始化为 `0`.
2. 在向远端发送消息之前，将本地计数器加一。
3. 在从远端收到消息之后，将本地计数器设置为其当前值与收到值中的最大值，再加 `1`.

伪代码如下：

```ts
const localTimestamp = 0;

function sendState(state) {
  send({ state, timestamp: ++localTimestamp });
}

function receiveState() {
  const { state, timestamp } = receive();
  localTimestamp = max(localTimestamp, remoteTimestamp) + 1;
}
```

### 冲突解决 <a href="#crdtconflicts" id="crdtconflicts"></a>

当 CRDT 遇到两个竞争状态时，它需要一种所有参与方都以相同方式应用的共享解决策略，以确保每个人都得出相同的决定。Decentraland 协议使用非常简单的规则：

1. 如果实体已被删除（且 ID 从未被复用），则忽略新状态。
2. 如果之前没有状态，则保留新状态。
3. 如果新状态的时间戳更大，则保留新状态。
4. 如果新状态的时间戳更小，则保留旧状态。
5. 如果两个时间戳相同，则逐字节比较状态，并保留较小的值。

大多数情况将由规则 `1`, `2` 和 `3`.

### 初始同步

在场景开始运行自己的代码之前，运行时会用所有 [基础实体](/docs/contributor/contributor-zh/chang-jing-yun-xing-shi/basic-entities/) 及其组件的状态填充 CRDT。

在这次初始同步期间，只有运行时可以设置共享状态。在该过程完成之前，不允许场景做出修改。

### 消息协议 <a href="#messages" id="messages"></a>

世界浏览器与场景运行时之间的消息是以序列化二进制表示的结构。每条消息都带有一个指示类型和长度的头部，随后是针对每种消息类型的特定负载。

```
.----------------.--------------.---------------------------------.
| 长度: uint32 | 类型: uint32 |     负载: byte[length]       |
'----------------'--------------'---------------------------------'
╵         通用字段         ╵         与类型相关         ╵

```

CRDT 协议中有三种消息类型：

* [`PutComponent`](#putcomponent)
* [`DeleteComponent`](#deletecomponent)
* [`DeleteEntity`](#deleteentity)

请注意，没有 `CreateComponent` 或 `CreateEntity` 消息。CRDT 规则会自动创建之前未知的实体和组件，正如 [上文所述](#crdtstateautocreate).

#### `PutComponentMessage` <a href="#putcomponent" id="putcomponent"></a>

更新 `状态` 的 `组件` 在特定 `实体`上的状态，必要时在 CRDT 中创建未知的组件和实体。 [根据](#crdtconflicts) 解决冲突 `时间戳`.

```
.----------------.-------------------.-------------------------------------------.
| 实体: uint32 | 组件: uint32 | 时间戳: uint32 |     状态: byte[]     |
'----------------'-------------------'-------------------------------------------'
╵                       通用字段                    ╵  与组件相关  ╵

```

该 `状态` 字段是由组件定义的二进制序列化。大多数组件使用 [Protocol Buffers](https://protobuf.dev) 来按照 `.proto` 文件中的规范对消息进行编码， [Decentraland 协议包中的](https://github.com/decentraland/protocol)。值得注意的例外是 `变换` 组件，它是迄今为止最常见的组件，因此拥有优化过的序列化格式。

这层额外的序列化有一个重要优点：CRDT 协议对当前或未来的任何组件实现都保持中立。

如果这条消息中包含的更新被应用到 CRDT 中，那么 `状态` 字段会原样复制到结构中。

#### `DeleteComponentMessage` <a href="#deletecomponent" id="deletecomponent"></a>

移除 `组件` 的状态，针对一个 `实体`. [根据](#crdtconflicts) 解决冲突 `时间戳`.

```
.----------------.-------------------.-------------------.
| 实体: uint32 | 组件: uint32 | 时间戳: uint32 |
'----------------'-------------------'-------------------'

```

#### `DeleteEntityMessage` <a href="#deleteentity" id="deleteentity"></a>

删除 `实体` （即所有相关的组件状态），并期望该标识符永远不会被复用于其他实体。

```
.----------------.
| 实体: uint32 |
'----------------'

```

请注意，没有 `时间戳` 字段。由于 `实体` 的生命周期已经结束，对任何乱序更新的冲突解决策略就是直接忽略它们。

## 方法

中的方法集合 `EngineApi` 提供了交换消息以及从头重建世界状态的接口。

### `crdtSendToRenderer` <a href="#crdtsendtorenderer" id="crdtsendtorenderer"></a>

将一条序列化消息从场景发送到渲染器，并返回渲染器为该场景准备的序列化消息数组。

```ts
接口 Request {
  // 此请求的序列化消息：
  data: byte[];
}

interface Response {
  // 来自渲染器的序列化消息数组（如果有）：
  data: byte[][];
}

function crdtSendToRenderer(Request): Promise<Response>;
```

### `crdtGetState` <a href="#crdtgetstate" id="crdtgetstate"></a>

```ts
interface Request {}

interface Response {
  // 状态是否包含场景创建的实体：
  hasEntities: boolean;

  // 可用于重建 CRDT 状态的消息数组：
  data: byte[][];
}

function crdtGetState(Request): Promise<Response>;
```

## 旧式场景

旧式场景（即使用 SDK 6 版或更早版本构建的场景）不使用现代 CRDT 机制，而是调用 `EngineApi` 中那些现在已被弃用的方法。

对这些场景的支持不是符合协议的 Decentraland 应用的必需要求。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-zh/chang-jing-yun-xing-shi/yun-xing-shi-mo-kuai/engine-api.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
