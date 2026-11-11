> 配置任何物品，使其表现得像一个智能物品。

# 让任何物品都变成智能物品



<div class="video-embed"><button type="button" class="video-play" data-video-src="https://www.youtube-nocookie.com/embed/wnnEU8GCLjc" aria-label="Play YouTube video"><svg viewBox="0 0 64 64" width="64" height="64" aria-hidden="true"><circle cx="32" cy="32" r="30" fill="currentColor"/><path d="m26 20 20 12-20 12Z" fill="#181214"/></svg><span>Play video</span></button></div>
<p class="video-source"><a href="&lt;https://www.youtube.com/watch?v=wnnEU8GCLjc>">Watch on YouTube</a></p>



智能物品只是带有一个 **动作** 和/或 **触发器** 组件。你可以将这些组件添加到场景中的任何物品上。你也可以导入自己的自定义 3D 模型，并为它们添加同样的组件。

要向物品添加组件，请点击 **加号图标** 位于物品名称旁边，然后从下拉列表中选择要添加的组件。

![实体上的“添加组件”菜单会打开，列出按 3D 内容和交互分组的组件，其中包括动作和触发器。](/docs/_remote/e0e52c0c060b63e97e2466dd.webp)

这带来了大量创意可能性。你可以把蜡烛变成一个杠杆，打开书架后面的秘密通道，从井里播放神秘声音，把钻石变成可收集物品，点击后缩小到 0。组合这些机制的方法有无数种富有想象力的做法！

## 交互性

你可以让物品对玩家的不同动作作出反应。


> [!NOTE]
> **💡 提示**：当玩家与物品交互时，你应该提供某种反馈，以让这种交互更清晰。如果模型没有任何动画，至少考虑播放一个声音。在某些情况下，让物品在缩放上做一个轻微的缓动后再回到原始大小，也可以作为反馈形式。


添加一个 **触发器** 组件，用于检测玩家的不同动作：

* **指针事件**：当玩家将光标对准物品时点击或按下按键。
* **全局按钮事件** 当玩家按下某个按键时，无论他们在场景中的什么位置。
* **玩家接近**：当玩家走到物品所在位置时。

该 **触发器** 组件可以配置为识别这些类型中的任意一种触发器。每次触发发生时，它都可以调用来自其自身 **操作** 组件，或来自 **操作** 场景中其他物品的组件。参见 [智能物品——高级](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/smart-items-advanced/).


> [!NOTE]
> **💡 提示**：你也可以将这些触发器与 [条件逻辑](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/states-and-conditions/)结合使用，这样动作不会在每次触发发生时都被调用，而只会在条件成立时调用。
> 
> 例如，你可以添加一个 **指针事件** 触发器到一扇门上，这样它会在点击时打开，但加入条件逻辑，使其只有在未上锁时才会打开。


### 指针事件

添加一个 **触发器** 组件，使用 **点击时** 或 **输入动作时** 触发事件。

* **点击时** 当玩家指向该物品时，每次点击鼠标左键都会触发。
* **输入动作时** 当玩家指向该物品时，每次按下主按钮（E）都会触发。

![](/docs/_remote/342cba6acbd2ae9ce5e8ce5f.webp)

**碰撞体**

重要的是，要使某个物品可点击，它必须具有一个 **碰撞体**。否则，你的点击会直接穿过模型，并尝试与后面的内容交互。默认资源包中的 3D 模型都应该有碰撞体，但如果你创建自己的模型，或从其他地方获取模型，可能会发现它缺少碰撞体。

如果你的模型缺少碰撞体，以下任一方法都可以修复：

* 添加一个 **网格碰撞体** 组件。这将创建一个具有 [基础几何体形状](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/shape-components/#primitive-shapes) （立方体、平面、圆柱体、球体）。
* 更改……的属性 **碰撞** 部分中的 **GLTF** 组件。 **可见层** 应分配给 **指针**.
* 在 Blender 中编辑 3D 模型，以包含一个不可见的碰撞几何体（任何名称以……结尾的网格 `_collider`）。参见 [碰撞器](/docs/creator/content-creator-zh/3d-jian-mo-yu-dong-hua/colliders/).


> [!NOTE]
> **💡 提示**：如果你使用了 **Mesh Renderer** 组件为模型赋予了一个原始形状，仅此并不会给它添加碰撞体。你还必须为它分配一个 **网格碰撞体** 组件，则抛出错误。


**自定义指针事件**

当物品具有一个 **点击时** 或一个 **输入动作时** 触发动作时，你可以覆盖所使用的默认设置。

* **悬浮文本**：更改玩家将光标悬停在物品上时在光标旁看到的提示。这对于说明你的物品的作用非常有帮助。
* **最大距离**：玩家在多远的距离内可以与物品交互。
* **显示反馈**：如果未勾选，当玩家将光标移到物品上时，该物品不会显示悬停提示。
* **按钮**：如果使用 **输入动作时** 触发动作，你可以重新分配默认的 **主按钮（E）** 到其他按键。悬停提示会包含一个图标，以说明应使用哪个按键。你可以使用 **副按钮（F）**，或 **动作 3 到 6** （数字键 1 到 4）。

### 全局按钮事件

添加一个 **触发器** 组件，使用 **全局点击时**, **全局主按键时** 或 **全局次按键时** 触发事件。

* **全局点击时** 当玩家在场景中的任何位置点击鼠标左键时都会触发。
* **全局主按键时** 当玩家在场景中的任何位置按下主按钮（E）时都会触发。
* **全局次按键时** 当玩家在场景中的任何位置按下副按钮（F）时都会触发。


> [!NOTE]
> **💡 提示**：通常将其与 [状态和条件](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/states-and-conditions/)结合使用是有意义的，这样只有当玩家在房间里，或满足其他条件时，物品才会对按钮事件作出反应。


### 玩家位置

添加一个 **触发器** 组件，使用 **玩家进入区域**, **玩家离开区域** 触发事件。

当玩家进入或离开一个默认大小为 1x1x1、位于物品中心的区域时，这将触发反应。


> [!NOTE]
> **💡 提示**：通常最好使用 [**触发区域**](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/smart-items/#trigger-areas) 智能物品，因为该物品的尺寸可以在场景编辑器中清晰可视化。


## 另请参见

* [智能物件 - 基础](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/smart-items/)
* [智能物品——高级](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/smart-items-advanced/)
* [状态和条件](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/states-and-conditions/)
* [与代码结合使用](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/shi-yong-dai-ma-kuo-zhan/overview/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/make-any-item-smart.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
