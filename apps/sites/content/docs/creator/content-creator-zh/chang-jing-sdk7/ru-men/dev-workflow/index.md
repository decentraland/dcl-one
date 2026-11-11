> 使用 Creator Hub 创建 Decentraland 场景的推荐工作流，从构思到测试和发布。

# 开发工作流

本文概述了为 Decentraland 开发场景时建议的步骤，从构思到发布及之后的流程。

## 我该如何创建一个 Decentraland 场景？

1. **安装** [**Creator Hub**](https://decentraland.org/download/creator-hub)，这是用于创建、预览和发布场景的官方桌面应用。它是适合各种技能水平创作者的推荐工具。
2. **如果你使用 AI 编码助手** （例如 Claude Code、Cursor 或 Copilot），请安装官方 [Decentraland SDK Skills](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/vibe-coding/) ，这样它就能识别经过验证的 SDK7 模式： `npx skills add decentraland/sdk-skills`
3. **该** [**CLI**](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/using-the-cli/) 是面向高级用户和自动化工作流的另一种选择。

下面的各个部分会详细介绍该工作流程的每个阶段。

## 安装 Creator Hub

请确保已安装 Decentraland Creator Hub。

* [安装指南](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/ru-men/editor-installation/)

如果你打算使用代码，也请确保安装 [Visual Studio Code](https://code.visualstudio.com/) 或 [Cursor AI](https://www.cursor.com/).


> [!NOTE]
> **💡 提示**：你也可以使用 AI 助手根据自然语言描述生成场景代码。参见 [使用 AI 进行氛围编程](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/vibe-coding/) ，了解如何开始使用 AI 辅助开发。


## 设计你的体验

考虑你需要占用多大的空间、采用什么样的分布、希望玩家能够执行哪些机制等。以下文档可作为指南：

* [UX 与 UI 指南](/docs/creator/content-creator-zh/chang-jing-sdk7/ti-yan-she-ji/ux-ui-guide/)
* [游戏设计约束](/docs/creator/content-creator-zh/chang-jing-sdk7/ti-yan-she-ji/design-games/)
* [场景 MVP 指南](/docs/creator/content-creator-zh/chang-jing-sdk7/ti-yan-she-ji/mvp-guidelines/)

## 发布到哪里

在 Decentraland 中，内容会发布到相邻的地块上，空间是有限的。玩家可以自由地从一个地块走到另一个地块。每个场景都是一个独立的小世界，一个场景中的物品不能延伸到另一个场景中，而且每个场景的代码都与其他场景隔离在沙盒中。

发布到这些位置的权限通过代币进行控制。开发场景不需要土地，但一旦准备发布，你就需要获得土地访问权限。

或者，你也可以选择发布到 Decentraland [Worlds](/docs/creator/content-creator-zh/chang-jing-sdk7/fa-bu/publishing-options/#decentraland-worlds)，它们是独立且隔离的场景。

可用的选项有：

* 租用 LAND
* 购买 LAND
* 从土地所有者处获取权限
* 发布到 Decentraland World，参见 [worlds](/docs/creator/content-creator-zh/chang-jing-sdk7/fa-bu/publishing-options/#decentraland-worlds) 了解更多。

请参阅 [发布选项](/docs/creator/content-creator-zh/chang-jing-sdk7/fa-bu/publishing-options/) 以了解更多详情。

## 模板和示例

创建新场景时，可从多个基础模板场景中进行选择，这些模板包含一些基础代码和 3D 模型。利用它们可以更快上手。

* [示例场景](/docs/creator/content-creator-zh/jiao-cheng-yu-shi-li/examples/)：在这里你可以找到精选的可运行场景和仓库集合，每个都展示了可供你复用或参考的机制。
* [辅助库](https://studios.decentraland.org/resources?sdk_version=SDK7&resource_type=Library)：它们可以简化许多常见任务。

## 美术资源

如果你是有经验的美术师，或者能请到有经验的人，你可以创建自定义 `.gltf` 或 `.glb` 模型用于你的场景。参见 [3D 模型基础](/docs/creator/content-creator-zh/3d-jian-mo-yu-dong-hua/3d-models/) ，了解如何为 Decentraland 创建 3D 模型的技巧。

不过，你不需要自己创建资源。参见 [实用资源](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/useful-resources/) ，获取可用于寻找 3D 模型的资源库和生成式 AI 工具，以及其他可加快工作流程的工具。

## 运行本地预览

要运行场景预览，请在 Creator Hub 中打开它并点击 **播放** 按钮。或者，如果你使用命令行，请运行 `npm run start` 在项目根目录中。

* [预览你的场景](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/preview-scene/) 以了解更多详情。
* 查看 [调试场景](/docs/creator/content-creator-zh/chang-jing-sdk7/tiao-shi/debug-in-preview/) ，获取有关如何调试任何问题的提示。
* 启用 **压缩资源** 在 **桌面客户端** 的子菜单 **播放选项** （或者运行 `npm run start -- --local-ab` （在 CLI 中）以将你的 3D 模型作为优化后的资源包加载。预览会运行得更流畅，并按发布后的效果显示模型。参见 [使用优化后的资源预览](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/preview-scene/#preview-with-optimized-assets).


> [!NOTE]
> **💡 提示**：使用 Creator Hub 时，每次你对场景进行更改，预览都会自动更新，即使在运行中也是如此。


## 发布到 Decentraland

当你对场景满意后，就可以将其发布到 Decentraland 了。为此，你需要拥有 LAND、Decentraland NAME 或 ETH ENS 名称，或者获得拥有这些权限的人授予的权限。

请参阅 [发布](/docs/creator/content-creator-zh/chang-jing-sdk7/fa-bu/publishing/) 以了解操作说明。

或者，你也可以发布到 [Worlds](/docs/creator/content-creator-zh/chang-jing-sdk7/fa-bu/publishing-options/#decentraland-worlds)，一个不需要 LAND 的个人 3D 空间。

## 推广

现在你的场景已经上线了，快把消息传出去吧！下面是一些方式：

* 在社交媒体上分享（#DCL）
* 在……上宣布 [Discord](https://dcl.gg/discord)
* 提交以便在……上展示 [events.decentraland.org](https://events.decentraland.org/)
* 在你的场景中组织活动
* 在一个高流量区域添加一个通往你场景的出生点

## 迭代

当你的场景上线一段时间并收到玩家反馈后，你就很适合对它进行迭代了！

用改进和新功能更新你的内容，并将场景的新版本部署到相同坐标。

## 回馈

如果你创建了一个令你自豪的场景、游戏或应用，不妨考虑将其开源！这样其他人可以从你的代码中学习并在你的工作基础上继续构建。你也可以在 [Awesome Repository](https://github.com/decentraland-scenes/Awesome-Repository).

如果你开发了一个可复用的功能模块，你也许会想把它做成一个库，让其他人可以导入到他们的项目中。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/ru-men/dev-workflow.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
