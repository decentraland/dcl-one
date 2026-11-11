> 将你的场景从 Web Editor 迁移到 Creator Hub。

# 迁移到 Creator Hub

如果你的场景是用 Creator Hub 之外的其他工具创建的，你可以轻松将其迁移到 Creator Hub。

Creator Hub 是创建 Decentraland 场景的推荐工具。它的界面比 Web Editor 更加完善，并且允许你将简单的拖放界面与通过代码进一步自定义的能力结合起来。它还允许你使用最新的桌面客户端运行场景预览。

## 从 Web Editor 迁移

要编辑使用 Web Editor 创建的场景中的代码，你必须将场景导出到你的电脑上，并用 Creator Hub 打开它。


> [!WARNING]
> **📔 注意**：如果你尚未安装 Creator Hub，请在开始之前先按照下一页中的步骤操作。
> 
> [安装 Creator Hub](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/ru-men/editor-installation/)


1. 点击 **下载图标** 在编辑场景时位于 Web Editor 顶部菜单中。

![](/docs/_remote/013ac41c9f6c08ef06e89d47.webp)

2. 这将下载一个 *.zip* 文件，解压它。
3. 打开 **创作者中心**，然后进入 **场景** 部分。
4. 点击 **导入场景** 按钮，并选择你导出的项目文件夹路径。

![](/docs/_remote/7cb4f59604e7cd5cbb0a15bd.webp)

完成后，你就可以在 Creator Hub 中继续进行项目开发，它的可视化界面与 Web Editor 很像，但更加完善。

你也可以编辑以下位置中的文件： `/src` 文件夹中的文件，为你的场景添加代码行为。请参阅 [与代码结合使用](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/shi-yong-dai-ma-kuo-zhan/overview/) 以了解如何编辑你的场景代码。

## 迁移纯代码项目

你可以将任何纯代码项目导入 Creator Hub。为此，

1. 打开 Creator Hub，进入 **场景** 部分。
2. 点击 **导入场景** 按钮，并选择你导出的项目文件夹路径。

![](/docs/_remote/7cb4f59604e7cd5cbb0a15bd.webp)

完成后，你就可以在 Creator Hub 中开始处理你的项目，这并不妨碍你继续使用自己喜欢的代码编辑器来编辑场景代码，或者使用命令行来运行或部署场景。

导入项目后，任何通过代码创建的内容都不会在 Creator Hub 画布上显示或可编辑，这会让放置和对齐新元素变得有些困难。你最初会看到你的场景是一个空网格。

![](/docs/_remote/3b0a67be118d41cad070cf4b.webp)

你无需从头手动将内容添加到画布上，可以运行一条命令自动为你添加。为此，请确保你已安装最新版本的 SDK，并在终端中运行以下命令：

```
npx sdk-commands code-to-composite
```


> [!CAUTION]
> **❗警告**：在运行此命令之前，请确保你已备份项目。
> 
> 此命令将覆盖 `main.composite` 文件，并用新的快照覆盖它。它还会将 `.ts` 以下 `src` 文件夹中的文件里的所有代码注释掉。你需要取消注释这些代码，才能让它再次运行。


此命令会运行你的场景，并在第一帧对通过代码创建的内容进行快照。该快照会保存到 `main.composite` 文件中，Creator Hub 会使用该文件来显示你场景中的内容。你场景中的代码会被注释掉，以避免所有实体出现重复。

请注意，此命令只会捕获实体以及可在 Creator Hub 界面中表示的组件。它不会复制自定义组件，也不会还原执行逻辑的代码，或者通过代码创建的 UI 元素。要恢复任何被注释掉的行为，你需要编辑 `.ts` 以下 `src` 文件夹中的代码，并取消注释你需要的行。

你也可以重写部分代码，使其不再创建新实体，而是通过名称或标签引用现有实体来赋予它们行为。请参阅 [与代码结合使用](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/shi-yong-dai-ma-kuo-zhan/overview/) 了解如何在代码中获取这些实体。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-bian-ji-qi/ru-men/migrate-from-web.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
