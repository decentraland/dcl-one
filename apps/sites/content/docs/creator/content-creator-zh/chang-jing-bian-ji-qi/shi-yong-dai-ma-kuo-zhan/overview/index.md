> 将 Scene Editor 中创建的内容与代码编写的能力结合起来。

# 与代码结合



<div class="video-embed"><button type="button" class="video-play" data-video-src="https://www.youtube-nocookie.com/embed/55H37rygD7M" aria-label="Play YouTube video"><svg viewBox="0 0 64 64" width="64" height="64" aria-hidden="true"><circle cx="32" cy="32" r="30" fill="currentColor"/><path d="m26 20 20 12-20 12Z" fill="#181214"/></svg><span>Play video</span></button></div>
<p class="video-source"><a href="&lt;https://www.youtube.com/watch?v=55H37rygD7M>">Watch on YouTube</a></p>



Creator Hub 与自定义代码是创建内容的一个非常强大的组合。你可以使用画布以直观的方式自由地定位各个项目，然后编写代码，以完全自由地与这些项目交互。你甚至可以放置一个智能项目，它有自己的默认行为，并编写代码来响应该项目被激活时发生的事情。

例如，你可以利用现有的杠杆智能项目，它已经自带声音、动画和状态，并编写代码来检测杠杆何时被拉动，从而运行你自己的自定义逻辑。

参见 [在代码中引用项目](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/shi-yong-dai-ma-kuo-zhan/reference-items/) 了解如何在你的代码中按名称或按标签获取项目。

## 编辑代码

你必须在你的计算机上安装一个代码编辑器，才能编辑场景的代码。推荐的选项有：

* <img src="/docs/_remote/358795a1c08dfc479355a174.webp" alt="VS Code" data-size="line"> [Visual Studio Code](https://code.visualstudio.com/)：这是有经验的开发者推荐的选项。
* <img src="/docs/_remote/9510a847d1493f0d8b4cf7e3.webp" alt="Cursor" data-size="line"> [Cursor AI](https://www.cursor.com/)：这是一个与 AI 集成的强大代码编辑器。它让你可以选择不同的 AI 模型来帮助你编写代码；提供免费层级，更高级的模型需要付费方案。对于刚接触 Decentraland 或 TypeScript 的开发者来说，或者如果你想节省编写代码的时间，这是一个不错的选择。


> [!WARNING]
> **📔 注意**：如果你使用的是 macOS，请确保代码编辑器应用位于 Applications 目录中。


安装完成后，你可能需要在 Creator Hub 的设置中选择你的代码编辑器。为此，

1. 点击屏幕左上角的 Creator Hub 标志并选择 **设置**.
2. 打开 **编辑器** 选项卡。
3. 在 **首选代码编辑器**，选择你的代码编辑器。你可能会在下拉菜单中看到你的编辑器，或者你可能需要选择 **从你的设备中选择...** 来找到它。

![编辑器选项卡中的应用偏好对话框，显示“首选代码编辑器”下拉菜单设置为 Visual Studio Code。](/docs/_remote/be0b7d493dc2e145af5207ac.webp)

## 打开场景的代码

在你的计算机上安装代码编辑器并在 Creator Hub 的设置中选择它后，你可以点击 **< > 代码** 按钮，打开你的场景项目。

![](/docs/_remote/b1e812c7bf7175e15d141046.webp)

这会在一个单独的窗口中打开代码编辑器。在左侧边栏中，你可以浏览项目的文件和文件夹结构。

![](/docs/_remote/89a525be913ad9dc95c15692.webp)

将你的自定义代码添加到 `index.ts` 位于 `/src`中，在 `main()` 函数里。否则，你也可以在该函数外添加自定义代码，或者创建新的 `.ts` 文件，放在 `/src` 文件夹中，但这些文件必须以某种方式在 `main()` 的函数中被引用 `index.ts`.


> [!WARNING]
> **📔 注意**：如果你已经安装了 VS Code 或 Cursor，但 **代码** 按钮无法打开它，可能是因为 VS Code 在你的机器上没有正确配置为可通过命令行打开。在大多数情况下，这在默认安装过程中会自动处理，但如果没有，请参见 [来自 VS 的这些说明](https://code.visualstudio.com/docs/setup/mac#_launching-from-the-command-line) 以启用命令行中的 VS Code。


如果你打开了一个预览窗口来运行你的场景，那么每当你更改文件中的代码并保存时，场景都会自动重新加载并应用你的更改。

## 使用 AI

Creator Hub 内置了一个 [AI 助手](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/shi-yong-dai-ma-kuo-zhan/ai-assistant/) AI 助手，它可以看到你打开的场景并为你编辑，而无需离开应用。它是一个实验性功能，默认处于关闭状态。

你也可以利用 Cursor 或 Claude Code 等 AI 助手来帮助你编写场景代码。例如，要使用 Cursor，请执行以下操作：

1. 通过点击以下按钮打开 Cursor AI 助手 **AI** 按钮，位于屏幕右上角
2. 在那里，你可以向 AI 助手发出提示来帮助你编写代码。你的提示可以包含文档页面链接、项目中特定文件的路径，甚至图片。你还可以从下拉菜单中选择要使用的特定模型。

为了让你的 AI 助手具备 Decentraland 上下文，请安装官方的 Decentraland SDK Skills。这些是由 Decentraland 基金会维护的现成指令集，教会你的 AI 代理针对每个主题使用经过验证的 SDK7 模式：搭建完整场景、添加 3D 模型、多人、部署等等。

```bash
npx skills add decentraland/sdk-skills
```

参见 [Vibe Coding with AI](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/vibe-coding/) 完整指南，包括可用技能的完整列表。


> [!WARNING]
> **📔 注意**：旧的场景项目可能包含一个 `/dclcontext` 文件夹，由已弃用的 `npx sdk-commands get-context-files` 命令下载。该上下文不再维护，已被 SDK Skills 取代。你可以安全地删除该文件夹。


## 版本控制

我们建议你在 GitHub 上为你的项目创建一个仓库，并用它来跟踪项目版本并与他人协作。

如果你不熟悉如何操作，请参见 [仓库快速入门](https://docs.github.com/en/repositories/creating-and-managing-repositories/quickstart-for-repositories)，或使用 [GitHub 桌面应用程序](https://desktop.github.com/download/) 以获得更简单的基于界面的流程。


> [!WARNING]
> **📔 注意**：将整个项目文件夹上传到 GitHub 仓库，但确保 `/node_modules` 或 `/bin` 文件夹以及 `package-lock.json` 文件都包含在 `.gitignore` 文件中，以避免同步它们。如果你将仓库配置为 `node`类型，则应当如此。这些文件都是自动生成的，其内容可能因不同机器而异。


## 另请参见

* [Vibe Coding with AI](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/vibe-coding/)：通过向 AI 助手描述你想要的内容来构建场景。
* [智能物件 - 基础](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/smart-items/)
* [智能物品——高级](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/smart-items-advanced/)
* [状态和条件](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/states-and-conditions/)
* [让任何物品变得智能](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/make-any-item-smart/)
* [SDK 快速入门](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/sdk-101/)：跟随这个简短教程快速入门。
* [开发工作流](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/dev-workflow/)：阅读此内容，了解从头到尾的场景创建流程。
* [示例](/docs/creator/content-creator-zh/jiao-cheng-yu-shi-li/examples/)：直接进入可运行的示例场景。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-bian-ji-qi/shi-yong-dai-ma-kuo-zhan/overview.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
