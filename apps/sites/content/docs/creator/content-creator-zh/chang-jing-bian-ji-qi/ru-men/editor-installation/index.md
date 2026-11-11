> 如何安装场景编辑器。

# Creator Hub 安装

![创作者中心](/docs/_remote/9100f2b541affb1c6fecfe89.webp)

下载 Creator Hub [这里](https://decentraland.org/download/creator-hub).

## 正在更新 Creator Hub

Creator Hub 应用每次打开时都会检查更新，如果有新版本可用，它会自动更新。

你也可以手动检查更新。为此，

1. 点击屏幕左上角的 Creator Hub 标志，打开主菜单。
2. 点击 **检查更新**.

![从左上角的标志打开的 Creator Hub 主菜单，其中包含关于 Creator Hub、检查更新、设置、报告错误和帮助与支持选项。](/docs/_remote/1394f3aa47147ab15685f4dd.webp)

主菜单也是你找到应用程序 **设置**以及报告错误或获取支持链接的地方。

## 编辑代码

如果你还计划在场景中阅读和编辑代码，你还需要安装以下之一：

* <img src="/docs/_remote/358795a1c08dfc479355a174.webp" alt="VS Code" data-size="line"> [Visual Studio Code](https://code.visualstudio.com/)：这是有经验开发者推荐的选项。
* <img src="/docs/_remote/9510a847d1493f0d8b4cf7e3.webp" alt="Cursor" data-size="line"> [Cursor AI](https://www.cursor.com/)：这是一款与 AI 集成的强大代码编辑器。它允许你选择不同的 AI 模型来帮助你编写代码；提供免费套餐，更高级的模型需要付费方案。这对于刚接触 Decentraland 或 TypeScript 的开发者来说是个不错的选择，或者当你想节省编写代码的时间时也很适合。

你可能需要在 Creator Hub 的设置中选择你的代码编辑器。为此，

1. 点击屏幕左上角的 Creator Hub 标志，并选择 **设置**.
2. 打开 **EDITOR** 选项卡中。
3. 在 **首选代码编辑器**下，选择你的代码编辑器。你可能会在下拉菜单中看到你的编辑器，或者你可能需要选择 **从你的设备中选择...** 来找到它。

![编辑器选项卡中的应用偏好设置对话框，显示将“首选代码编辑器”下拉菜单设置为 Visual Studio Code，以及“播放选项”复选框和“应用警告”复选框。](/docs/_remote/be0b7d493dc2e145af5207ac.webp)

### AI 技能

如果你打算使用 AI 代理来帮助你编写代码，我们建议安装 Decentraland SDK 技能。这些是现成的指令集，可教你的 AI 代理如何使用 Decentraland SDK，从而减少错误，并从第一条提示开始就提供更好的结果。要安装全部技能，请在你的场景项目文件夹中运行以下命令：

```bash
npx skills add decentraland/sdk-skills --all
```

参见 [Vibe 编程](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/vibe-coding/) 以了解有关安装特定技能以及如何使用它们的更多详情。

## 故障排除

如果你遇到问题，请参阅 [故障排除](/docs/creator/content-creator-zh/chang-jing-sdk7/tiao-shi/troubleshooting/) 部分。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-bian-ji-qi/ru-men/editor-installation.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
