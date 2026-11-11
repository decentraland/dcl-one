> 对玩家的位置做出反应

# 触发区域

要在玩家走进或走出某个区域时触发某事，请使用触发区域 [智能物品](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/smart-items/).

![](/docs/_remote/9dcff76b0c7616640d8295ac.webp)

在编辑场景时看到的橙色立方体只会在场景编辑器中可见，在预览场景运行时它会变成不可见。你可以轻松调整并缩放橙色立方体，使其恰好覆盖你需要的区域。

拖到你的场景中，并用缩放操纵器调整大小。你看到的盒子或球体只会在编辑器中绘制，玩家永远看不到它。

如果玩家的化身有任何部分与这个橙色立方体重叠，将调用所分配的事件。触发区域只对本地机器上的玩家有反应，不会对其他玩家的化身有反应——每个玩家都在自己场景实例中触发该区域。


> [!NOTE]
> **💡 提示**：如果你场景中的触发区域开始妨碍编辑其他内容，记住你总是可以从 [实体树](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/ru-men/scene-editor-essentials/#the-entity-tree).
> 
> <img src="/docs/_remote/c1baca1d0d660705a098e478.webp" alt="" data-size="original">


多个触发区域可以重叠，并且彼此不会相互影响。

你描述应该发生什么，而 [AI 助手](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/shi-yong-dai-ma-kuo-zhan/ai-assistant/) 会为此编写一个小脚本。

### 设置

该物品的 **脚本** 组件有两个字段：

| 字段       | 作用          | 选项                                                        | 默认值    |
| -------- | ----------- | --------------------------------------------------------- | ------ |
| **激活方式** | 是什么触发该区域。   | `我的玩家`, `所有玩家`, `任何碰撞体`, `可点击`, `自定义 1`, `自定义 2`, `自定义 3` | `我的玩家` |
| **形状**   | 该体积是盒子还是球体。 | `盒子`, `球体`                                                | `盒子`   |

`我的玩家` 仅对运行该场景的玩家化身作出反应。 `所有玩家` 会对任何化身作出反应，包括场景中的其他人。其余选项对应其他 [碰撞层](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/colliders/)，因此你可以让该区域对物理对象、可点击对象或你自己的自定义层作出反应。

切换 **形状** 会在编辑器占位符在盒子和球体之间切换，因此你编辑时看到的体积就是最终获得的体积。

### 描述应该发生什么

该区域会触发两个事件： **进入** 和 **离开**。在 **反应** 部分中，每个事件都有一个按钮：

* **当玩家进入……**
* **当玩家离开……**

点击其中一个，Creator Hub 会把一句话的开头写入 AI 助手的消息框，并命名该物品，例如 `当玩家进入“触发区域”时，` 提示词使用你在实体树中看到的物品名称。用你希望发生的内容补全句子并发送。助手会编写一个单独的脚本，并将其附加到同一实体上，与检测器并列。

按钮旁边的三个标签（**播放声音**, **显示消息**, **获得分数**）会为你补全成完整句子，所以你可以直接发送。


> [!NOTE]
> **💡 提示**：反应需要 [AI 助手](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/shi-yong-dai-ma-kuo-zhan/ai-assistant/) 处于开启状态。如果没有开启，Creator Hub 会告诉你在哪里启用它，而不是打开聊天。



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/trigger-area.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
