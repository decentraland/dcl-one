> 使用 AI 助手将旧版 SDK6 场景迁移到 SDK7。

# 迁移旧版 SDK6 场景

如果你有一个旧的 SDK6 场景，希望将其更新到最新版本，你可以借助 AI 助手自动将其迁移到 SDK7。该 [Decentraland SDK Skills](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/vibe-coding/#install-skills-for-any-ai-agent) 包含一个专门的迁移工作流，AI 会一步一步地遵循它，因此你不必手动翻译代码。

本指南将带你完成整个流程。

## 1. 创建一个新的空 SDK7 场景

从一个全新、空白的 SDK7 项目开始。AI 会将你的旧代码和资源迁移到这个干净的结构中。

**选项 A — Creator Hub**

1. 打开 Creator Hub。
2. 选择 **场景** 选项卡，并点击 **新场景**.
3. 选择 **空场景** 模板。

**选项 B — 命令行**

在一个空文件夹中运行以下命令：

```bash
npx @dcl/sdk-commands init
```

这会将 SDK7 项目的默认文件填充到该文件夹中。

## 2. 安装 SDK Skills

SDK Skills 会教你的 AI 代理如何使用 Decentraland SDK7，包括如何正确地执行 SDK6 → SDK7 迁移。

你可以通过两种方式安装它们。

**选项 A — 让 AI 安装它们**

在场景文件夹中打开你的 AI 助手，然后直接说：

> 安装 Decentraland SDK skills。

AI 知道这个命令，会为你安装它们。

**选项 B — 从命令行安装**

```bash
npx skills add decentraland/sdk-skills --all
```

该 `--all` flag 会安装所有可用的 skills。如果省略它，则会打开一个交互式选择器，让你选择要安装哪些 skills。

有关可用 skills 及其工作方式的更多详情，请参阅 [Vibe Coding with AI](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/vibe-coding/#install-skills-for-any-ai-agent).

## 3. 让 AI 迁移场景

将你的旧 SDK6 项目文件放在 AI 可以读取的地方——例如你机器上的一个单独文件夹，或一个公开的 GitHub 仓库中。

然后向 AI 提示：

> 将这个 SDK6 场景迁移到 SDK7。

这些 skills 包含完整的迁移操作手册，因此 AI 将会：

* 识别 SDK6 场景中使用的入口点和组件。
* 将实体、组件和系统转换为它们在 SDK7 中对应的版本。
* 将 3D 模型、音频和其他资源移动到新项目中。
* 更新 `scene.json` 以及项目配置。
* 用当前的 SDK7 对应 API 替换已弃用的 API。

## 4. 测试并迭代

迁移完成后，预览场景并完整体验一遍。

* 在 Creator Hub 中，点击 **播放**.
* 在命令行中运行 `npm run start`.

在测试时，请留意：

* 视觉问题——模型缺失、位置或缩放错误、材质损坏。
* 交互问题——点击无响应、触发器未激活、对话框未出现。
* 如果你的场景用了多人或联网功能，也要留意相关问题。
* 预览窗口中的控制台错误。

当你发现一个 bug 时，用通俗的语言把它描述给 AI：

> 桌子上的红色按钮不再能打开门了。它以前会播放声音并将门旋转 90 度。

AI 可以利用相关的 skills（交互、动画、音频等）逐一修复问题。不断迭代，直到场景表现得和原来一样。


> [!NOTE]
> **💡 提示**：一次修复一个 bug，并在每次改动后预览。改动越小，就越容易发现回归问题。


## 下一步

* [Vibe Coding with AI](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/vibe-coding/) — 与 AI 助手和 SDK Skills 协作的完整参考。
* [SDK 快速入门](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/sdk-101/) — 学习 SDK7 基础知识，以便你审查并完善迁移后的代码。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/qi-ta/migrate-legacy-sdk6-scenes.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
