# 基本实体

世界探索者 [预留一段范围](/docs/contributor/contributor-zh/chang-jing-yun-xing-shi/yun-xing-shi-mo-kuai/engine-api/#identifying) 的实体 ID 供其自身使用。在这段范围中，标识符 `0`, `1` 和 `2` 预先分配给一组保证存在的基础实体。

它们是：

* `0`: `RootEntity`
* `1`: `PlayerEntity`
* `2`: `CameraEntity`

这些实体具有一组已知的附加组件，由运行时管理，即它们的状态对场景而言是只读的。获取这些组件的初始状态是 [场景初始化](/docs/contributor/contributor-zh/chang-jing-yun-xing-shi/execution/#onstart).


> [!NOTE]
> 这些实体的附加组件正在开发中。 [CRDT 同步机制](/docs/contributor/contributor-zh/chang-jing-yun-xing-shi/yun-xing-shi-mo-kuai/engine-api/#synchronization) 正在扩展，以提供不同模块所提供的部分功能。
> 
> 例如，场景将能够检测 Explorer 窗口的状态，以优化其网络和 CPU 使用，或者访问有关玩家、通信系统和游戏摄像机的更丰富信息。


#### `RootEntity` (`0`) <a href="#rootentity" id="rootentity"></a>

包含有关场景上下文的一般信息。

* [`变换`](/docs/contributor/contributor-zh/chang-jing-yun-xing-shi/basic-components/#transform)：场景原点的位置信息，所有没有显式父级的其他变换都相对于此点。

该 `变换` 的组件 `RootEntity` 以以下值初始化：

```yml
scale: [1, 1, 1] # XYZ
position: [0, 0, 0] # XYZ
rotation: [0, 0, 0, 1] # XYZW
```

#### `PlayerEntity` (`1`) <a href="#playerentity" id="playerentity"></a>

包含有关玩家、其身份、其头像和位置的数据。

* [`变换`](/docs/contributor/contributor-zh/chang-jing-yun-xing-shi/basic-components/#transform)：玩家头像的位置信息。

#### `CameraEntity` (`2`) <a href="#cameraentity" id="cameraentity"></a>

包含有关游戏摄像机的信息。所有组件均由渲染器管理。

* [`变换`](/docs/contributor/contributor-zh/chang-jing-yun-xing-shi/basic-components/#transform)：摄像机本身的位置信息。
* [`指针锁定`](/docs/contributor/contributor-zh/chang-jing-yun-xing-shi/basic-components/#pointerlock)：指针是否跟随摄像机方向或独立移动。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-zh/chang-jing-yun-xing-shi/basic-entities.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
