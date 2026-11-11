> 使用 Decentraland CLI 运行和部署场景。对于大多数创作者来说，Creator Hub 是推荐工具；CLI 更适合高级和自动化工作流。

# 使用 CLI


> [!NOTE]
> **💡 提示**：如果你正在使用 AI 编码助手（如 Claude Code、Cursor 或 Copilot），请在它编写任何代码之前先安装官方 Decentraland SDK Skills，这样它就会知道经过验证的 SDK7 模式：
> 
> ```bash
> npx skills add decentraland/sdk-skills
> ```
> 
> 参见 [与 AI 一起进行 Vibe Coding](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/vibe-coding/) 以了解完整的 AI 辅助工作流程。


要为 Decentraland 构建场景，你可以使用以下任一方式：

* 该 [创作者中心](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/ru-men/editor-installation/)
* 命令行界面（CLI）

这两种工具都允许你在“链下”开发环境中编译并预览你的场景。在本地测试场景后，你可以将内容上传到内容服务器，并将其与你的 LAND 或 WORLD 关联。

虽然 Creator Hub 中的场景编辑器更易于使用，但 CLI 提供了更多灵活性，并且可以轻松用于自动化流程。


> [!WARNING]
> **📔 注意**：场景编辑器在幕后执行的是相同的命令行操作。



> [!NOTE]
> **💡 提示**：参见 [安装指南](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/ru-men/editor-installation/) 了解如何安装 Creator Hub 的说明。


## 开始之前

要通过命令行处理场景，请在对场景运行 CLI 命令之前安装以下依赖项：

* [Node.js](https://nodejs.org) （20 版或更高版本）

## 初始化新项目

运行 `npx @dcl/sdk-commands init` 在一个空文件夹中运行，以用 Decentraland 的默认文件填充它 [场景](/docs/creator/content-creator-zh/chang-jing-sdk7/xiang-mu-lei-xing/scene-metadata/) 项目。你也可以使用 **新建场景** 按钮在 Creator Hub 中创建一个新场景，这会生成相同的项目结构。

要从另一种项目类型开始，请使用 `--project` 标志。例如，要创建一个 [智能穿戴设备](/docs/creator/content-creator-zh/chang-jing-sdk7/xiang-mu-lei-xing/smart-wearables/) 项目：

```bash
npx @dcl/sdk-commands init --project smart-wearable
```

可用的 `--project` 选项有 `scene-template` （默认值）、 `px-template`, `smart-wearable`，以及 `library`.

## 更新场景的 SDK 版本

在场景文件夹中运行以下命令：

```bash
npm i @dcl/sdk@latest
```

你可以通过检查场景的 `package.json` 文件，并查看其中的 `@dcl/sdk` 版本来确认它是否生效。

## 运行预览

运行 `npm run start` 在场景、工作区或智能穿戴设备项目的根目录下运行，以在 Decentraland Desktop 客户端中打开预览。

```bash
npm run start
```

要在 [Decentraland 移动应用](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/yi-dong-ke-hu-duan/overview/) 中预览你的场景，请改为运行 `npm run start -- --mobile` （别名 `-- -m`）。CLI 会打印一个二维码，可在与你电脑连接到同一 Wi‑Fi 网络的手机上打开场景。参见 [在移动端预览](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/kai-fa/preview-on-mobile/) 获取完整指南。

```bash
npm run start -- --mobile
```

为了让预览运行更流畅，并渲染出与发布后完全相同效果的 3D 模型，请添加 `--asset-bundles` 标志。参见 [使用优化资源预览](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/preview-scene/#preview-with-optimized-assets).

```bash
npm run start -- --asset-bundles
```

参见 [预览场景](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/preview-scene/) 了解运行预览时的详细信息和特殊选项。

## 构建

运行 `npm run build` 来构建你的项目。Decentraland 场景用 TypeScript 编写，但在发布时会构建为压缩后的 JavaScript。参见 [编写场景](https://github.com/decentraland/docs/blob/main/creator/sdk7/getting-started/coding-scenes.md) 了解更多详情。

构建命令是可选的，因为它也会在部署前在后台运行（尽管你可以添加标志来跳过它）。

构建命令会进行比 `npm run start`更严格的类型检查，因此有时运行它有助于调试场景。

## 部署场景

运行 `npm run deploy` 将你的场景发布到 Decentraland。此命令会打开一个浏览器窗口，你可以在其中使用钱包签名以授权部署。

参见 [发布](/docs/creator/content-creator-zh/chang-jing-sdk7/fa-bu/publishing/) 了解发布场景时的详细信息和特殊选项。

## 故障排除

如果你遇到问题，请参阅 [故障排除](/docs/creator/content-creator-zh/chang-jing-sdk7/tiao-shi/troubleshooting/) 部分。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/ru-men/using-the-cli.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
