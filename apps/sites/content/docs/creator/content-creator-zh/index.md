> 创建 Decentraland 场景、可穿戴物和表情动作。推荐使用 Creator Hub，SDK Skills 可用于 AI 编码助手，CLI 适用于高级工作流。

# 欢迎，创作者

让我们一起构建 Decentraland！

欢迎所有创作者！在 Decentraland 中，你拥有广泛的创作可能性，适合不同才华和技能水平的人！

Decentraland 可在桌面端（Windows 和 macOS）以及移动设备（iOS 和 Android）上使用。玩家可以在使用同一账号、头像和库存的不同客户端之间无缝切换——这意味着你的场景需要在所有人身上都能良好运作，无论他们使用什么设备进行游玩。在本指南中，你会找到关于如何设计、构建和测试场景的建议，以确保它们在每个受支持的平台上都能表现出色。关于移动端的专门指导，请参见 [面向移动端构建](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/yi-dong-ke-hu-duan/overview/).

## 创作者中心

创作者中心是适合所有知识水平创作者的推荐工具。它是一款桌面应用程序，可让你创建 [场景](#scenes).

![](/docs/_remote/32e1b985101d928b718678f9.webp)

下载创作者中心 [此处](https://decentraland.org/download/creator-hub).

## 穿戴物

穿戴物是玩家头像可以穿戴的服饰物品。这些物品以 NFT 形式出售，并在 [市场](https://decentraland.org/marketplace/browse?section=wearables&vendor=decentraland&page=1&sortBy=newest&status=on_sale).

了解全部关于 [创建穿戴物](/docs/creator/content-creator-zh/chuan-dai-wu-yu-biao-qing-dong-zuo/chuan-dai-wu/creating-wearables/).

你还可以将穿戴物与 SDK 中的代码结合起来，创建一个 [智能穿戴物](/docs/creator/content-creator-zh/chang-jing-sdk7/xiang-mu-lei-xing/smart-wearables/)。这会在玩家穿上该穿戴物时启用一个全局场景。参见 [项目类型](/docs/creator/content-creator-zh/chang-jing-sdk7/xiang-mu-lei-xing/kinds-of-project/) 以更好地了解不同选项。

## 表情动作

表情动作是玩家头像可以做出的动画。这些物品以 NFT 形式出售，并在 [市场](https://decentraland.org/marketplace/browse?assetType=item&section=emotes&vendor=decentraland&page=1&sortBy=newest&status=on_sale).

了解全部关于 [创建表情动作](/docs/creator/content-creator-zh/chuan-dai-wu-yu-biao-qing-dong-zuo/biao-qing-dong-zuo/creating-emotes/).

## 场景

Decentraland 中的 3D 内容由多个场景组成，每个场景占据有限空间，并相邻显示，供玩家自由穿行。

创作者中心让你可以通过简单的拖放界面创建场景，还能编辑代码，从而对交互拥有完全控制权。你可以运行预览、调试、编辑代码并发布。

[了解更多](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/ru-men/about-editor/)

### 我该如何创建一个 Decentraland 场景？

1. **先安装** [**创作者中心**](https://decentraland.org/download/creator-hub)，这是用于创建、预览和发布场景的官方桌面应用。它是适合所有技能水平创作者的推荐工具。
2. **如果你使用 AI 编程助手** （例如 Claude Code、Cursor 或 Copilot），请安装官方的 [Decentraland SDK Skills](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/vibe-coding/) ，这样它就能了解已验证的 SDK7 模式： `npx skills add decentraland/sdk-skills`
3. **该** [**CLI**](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/using-the-cli/) 是高级用户和自动化工作流的替代方案。

### 3D 艺术

Decentraland 场景由 3D 模型组成。

* 可在场景编辑器中从丰富的默认资源目录中选择。这些资源已准备就绪，并针对在 Decentraland 中使用进行了优化

  ![](/docs/_remote/b193f61ea826d9783815e90e.webp)
* 使用 Blender 或你偏好的 3D 工具制作自己的 3D 模型。然后将它们导入场景编辑器。

  ![](/docs/_remote/6469c594dcf3e77e830e77da.webp)


> [!WARNING]
> **📔 注意**：Decentraland 中的内容应保持在一定的 [大小限制](/docs/creator/content-creator-zh/chang-jing-sdk7/you-hua/scene-limitations/) 之内，以确保你的场景运行流畅。
> 
> 参见 [3D 建模](/docs/creator/content-creator-zh/3d-jian-mo-yu-dong-hua/3d-models/) 获取有关优化的技巧和窍门，以及关于 3D 模型受支持功能和格式的信息。


### 交互性

要让你的场景具有交互性：

* **无需代码**：使用场景编辑器的界面将 [智能物品](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/smart-items/) 放入你的场景中。这些模型预置了自己的行为，而且高度可定制。你也可以为自己的自定义模型分配相同的行为（无需代码）。

  ![](/docs/_remote/a417642641d975327ba066bf.webp)
* **代码**：对于想要加入自定义逻辑的开发者，请使用 SDK 编写代码，实现你能想象到的任何功能。学习如何使用 SDK：
  * [SDK 快速入门](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/sdk-101/)：跟随这个简短教程快速入门。
  * [开发工作流](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/dev-workflow/)：阅读此内容，了解从头到尾的场景创建流程。
  * [Vibe Coding with AI](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/vibe-coding/)：通过用自然语言描述你想要的内容来构建场景，然后让 AI 编程助手为你编写代码。
  * [示例](/docs/creator/content-creator-zh/jiao-cheng-yu-shi-li/examples/)：直接进入可运行的示例场景。

    ![](/docs/_remote/3d5fae2810e08190c0c1ad38.webp)


> [!WARNING]
> **📔 注意**：你还需要拥有 [Visual Studio Code](https://code.visualstudio.com/) 已安装。


### 发布场景

开始使用场景编辑器构建你的场景时，你不需要拥有任何代币。要发布你的场景，你可以从以下选项中选择：

* **Genesis City 中的 LAND**：这是 Decentraland 中的主要开放世界，被划分为 16x16 米的地块。购买一个或多个相邻地块，位于 [市场](https://decentraland.org/marketplace/lands)，然后将你的场景部署在那里。
* **Decentraland Worlds**: [Worlds](/docs/creator/content-creator-zh/chang-jing-sdk7/fa-bu/publishing-options/#decentraland-worlds) 是你在元宇宙中的专属空间。你只需要拥有一个 [Decentraland 名称](https://decentraland.org/marketplace/names/claim)，就可以发布任意大小的场景！

参见 [项目类型](/docs/creator/content-creator-zh/chang-jing-sdk7/xiang-mu-lei-xing/kinds-of-project/) 以更好地了解不同选项。

参见 [发布](/docs/creator/content-creator-zh/chang-jing-sdk7/fa-bu/publishing/) 以了解在向 Genesis City 或 Worlds 发布场景时的详细信息和特殊选项。

## 实用资源

查看 [实用资源](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/useful-resources/) 精心整理的工具、插件、资源库和示例项目列表，这些内容可以加快你的创作工作流。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/readme.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
