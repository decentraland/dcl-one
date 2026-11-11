> 管理物品状态和条件逻辑

# 状态与条件



<div class="video-embed"><button type="button" class="video-play" data-video-src="https://www.youtube-nocookie.com/embed/wm8ZD2kSyKA" aria-label="Play YouTube video"><svg viewBox="0 0 64 64" width="64" height="64" aria-hidden="true"><circle cx="32" cy="32" r="30" fill="currentColor"/><path d="m26 20 20 12-20 12Z" fill="#181214"/></svg><span>Play video</span></button></div>
<p class="video-source"><a href="&lt;https://www.youtube.com/watch?v=wm8ZD2kSyKA>">Watch on YouTube</a></p>



## 条件逻辑

为触发器添加条件，使操作只有在满足这些条件时才会发生。例如，点击门只有在门尚未打开时才会激活“打开”操作。

要添加条件，请点击旁边的三点图标 **触发事件** 并选择 **添加触发条件**.

![](/docs/_remote/39d3afce89ca9fad61d21f61.webp)

一个触发器可以包含多个条件。点击 **+** 图标以添加更多条件。当存在多个条件时，你可以选择以下选项之一：

* **所有条件都应满足（AND）**：只有当每个条件都为真时，触发器才会发生。
* **任一条件可满足（OR）** 只要至少有一个条件为真，触发器就会发生。

![](/docs/_remote/95e6cb8369815d94d5f26f5a.webp)

### 状态

该 **状态** 该组件包含在多个智能物件中。它列出了智能物件可能处于的状态。在任意时刻，智能物件都处于这些状态之一。例如，门可以处于 *打开* 或 *关闭*。“打开”操作将状态设置为 *打开*，“关闭”操作将状态设置为 *关闭*.

你可以使用状态执行以下操作：

1. 在触发器上使用条件来检查实体的状态。这样，只有当特定状态处于活动状态时，才会执行该操作。

![](/docs/_remote/39d3afce89ca9fad61d21f61.webp)

2. 通过以下方式更改状态： **设置状态** 操作。

![](/docs/_remote/6f9bc751de8405306a6f58e0.webp)

3. 通过以下方式响应状态变化： **状态变化时** 触发事件。

要在两个操作之间切换，请定义两个触发器，每个触发器都带有一个检查状态的条件。例如，门有一个触发器用于激活“打开”操作，其条件首先检查门的状态是否为 *关闭*，另一个触发器用于激活“关闭”操作，其条件检查门的状态是否为 *打开*。每次玩家点击门时，只有其中一个会被激活。

![](/docs/_remote/819921eca04a4f69ab10b587.webp)

你可以为智能物件添加任意多个状态。只需点击 **添加新状态** 按钮即可向列表中添加另一个状态。

![](/docs/_remote/fb464b1832beb2b74eb0d3e9.webp)

其中一个状态会被选为默认状态，场景运行时该物件将始终以此状态开始。你可以通过点击另一个状态旁边的三点图标并选择 **设为默认**.


> [!NOTE]
> **💡 提示**：保持物件之间的交互简单。例如，避免某个按钮通过触发三个操作来打开一扇门：播放门的动画、播放门的声音并更改门的状态。相反，让按钮只更改门的状态。然后使用一个 **状态变化时** 触发器，这样门本身就会在状态变化时处理动画和声音的播放。


### 计数器

使用 **计数器** 组件用于跟踪一个数字，该数字会随着玩家在场景中执行操作而变化。你可以在条件逻辑中使用计数器的值。

当实体具有计数器组件时，你可以对其执行以下操作：

* **递增计数器**：按可配置的 **数量**增加计数器的值，默认是 1。
* **减少计数器**：按可配置的 **数量**增加计数器的值，默认是 1。
* **设置计数器**：将计数器的值设为某个特定数字，例如将其重置为 0。

使用 **计数器变化时** 触发器，以便在计数器值每次变化时执行一个操作。为此触发器添加条件，使其仅在超过某个阈值后才激活。

![](/docs/_remote/c4189a0d754c03116088f69e.webp)

在条件中，你可以检查计数器的值是否为

* 大于给定值
* 小于给定值
* 等于给定值


> [!NOTE]
> **💡 提示**：若要检查大于或等于，可以使用 AND 选项向触发事件添加两个条件。
> 
> 为了使某个操作在跨过阈值时只发生一次，而不是之后每次递增都重复发生，请将计数器与一个 **状态** 组件结合使用。每当达到所需值时，将状态设为“Done”，并在触发事件上添加一个条件来检查此状态。


### 另请参见

* [智能物件 - 基础](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/smart-items/)
* [智能物品——高级](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/smart-items-advanced/)
* [让任何物品变得智能](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/make-any-item-smart/)
* [与代码结合使用](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/shi-yong-dai-ma-kuo-zhan/overview/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/states-and-conditions.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
