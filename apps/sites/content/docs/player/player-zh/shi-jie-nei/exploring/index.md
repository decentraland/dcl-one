> 探索 Decentraland

# 探索

Decentraland 完全由社区生成的内容组成。当你外出探索时，你永远不知道会发现什么——无论是在 Genesis City 地图上，还是在某个人的 World 里！可选内容可能会让人眼花缭乱，所以这里有一些小贴士，帮助你充分体验这段冒险。

进一步了解 Genesis City 与 Worlds 之间的区别 [**这里**](/docs/player/player-zh/shi-jie-nei/overview/#genesis-city-vs-worlds-decentralands-landscape-explained).

## 探索 Genesis City

Genesis City 里有成千上万个由社区拥有的地块，直接一路走过去本身就是一场冒险。不过，如果你想更精确地探索，地图会非常有帮助。

### **使用地图发现新内容**

你可以点击屏幕左上角的小地图，或者点击侧边栏菜单中的地图图标来打开地图。

### **图层与图钉**

查看 Genesis City 的卫星图像时，你会注意到许多不同的图标。 **要更改地图类型并切换位置图钉显示**，请点击地图左下角的图层图标。

红色的“**现场活动**”图钉显示当前正在进行活动的位置。点击图标查看更多活动信息，然后点击“跳转进入”即可传送到其位置。

黄色的“**兴趣点**”图钉标记社区认为必看的地点——它们可以是任何东西，从游戏到令人印象深刻的艺术作品。点击该图标会弹出一段说明和一个“跳转进入”按钮，让你直接传送过去。

![](/docs/_remote/6f10d341d8bd755952cba253.webp)

### **地点类别**

根据类别在 Decentraland 中发现有趣的地点。打开地图后，你会在屏幕顶部看到各种类别气泡，包括艺术、音乐、游戏等主题。

点击某个类别后，会显示标记了该类别的地点图钉。点击其中一个会打开信息卡，里面有说明以及一个“跳转进入”按钮，这样你就可以直接传送到现场。

![](/docs/_remote/3b46b88f36b601ef41b01880.webp)

### **传送到特定地块**

在世界内，你可以使用 **地图** 或 **聊天命令**.

* **通过地图传送：** 点击地图上的任意位置都会弹出该地点的信息卡，即使那是一个空地块也一样。该卡片还会有一个“跳转进入”按钮，可将你传送到那个地点。
* **通过聊天命令传送：** 如果你知道想要跳转的地块坐标，请将以下命令输入屏幕左下角的附近聊天框，然后按下\[**回车**].
  * `/goto x,y` （例如 `/goto 0,0` ，前往 Genesis Plaza）
    * 进一步了解聊天命令 [**这里**](/docs/player/player-zh/shi-jie-nei/shortcuts-and-chat-commands/#chat-commands).

## 探索 Worlds

与 Genesis City 相比， **Worlds** 是更私密的虚拟空间，就像位于 Decentraland 地图之外的个人岛屿。它们只能通过传送访问。

### 发现新的世界

虽然 Worlds 往往更私密，但你可以通过浏览列在 [**Places 页面**](https://decentraland.org/places/worlds/) 上的 Worlds，以及参加在 Worlds 举办的活动来发现它们。

超过 1000 位 World 所有者选择将自己的 Worlds 公开列在 Places 页面上，使它们更容易被发现。浏览这些内容并按类别筛选，找到适合你日常冒险的完美 World。

![](/docs/_remote/c8ec811d9ea38b77c8b4ab58.webp)

World 中也可以举办活动——参加这些活动，你可以结识朋友并探索！下面是一个在 World 中举办的活动示例。通过红宝石色的“jump in”箭头，你可以看到 World 的名称取代了 Genesis City 的坐标。

![](/docs/_remote/2edf1e8080fe1670a08237b9.webp)

### 传送到一个 World

如果你知道某个 World 的名称，你可以使用一个 **聊天命令**。只需将以下命令输入屏幕左下角的附近聊天框，然后按下\[**回车**].

* `/goto World'sName` （例如 `/goto officehours`)
  * 进一步了解聊天命令 [**这里**](/docs/player/player-zh/shi-jie-nei/shortcuts-and-chat-commands/#chat-commands).


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/player/player-zh/shi-jie-nei/exploring.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
