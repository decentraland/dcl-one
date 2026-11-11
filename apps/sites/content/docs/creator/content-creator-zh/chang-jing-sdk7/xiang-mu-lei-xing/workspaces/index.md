> 同时运行多个 DCL 项目

# 工作区

通过将这些 Decentraland 项目分组到一个工作区中，在预览中运行多个项目。运行多个相邻场景，看看它们如何匹配，或者也可以运行多个 [智能穿戴物](/docs/creator/content-creator-zh/chang-jing-sdk7/xiang-mu-lei-xing/smart-wearables/) 一起运行，看看它们彼此之间以及与不同场景之间如何交互。

在工作区中运行多个项目可提供更完整的测试方式，以确保不同内容能良好协同工作。工作区是一项调试功能，不会影响已发布场景中的体验。


> [!WARNING]
> **📔 注意**： **创作者中心** 目前不支持处理工作区。


## 创建工作区

1. 下载 [Goerli 广场](https://github.com/decentraland/sdk7-goerli-plaza) 仓库。
2. 创建一个单独的顶级文件夹来存放工作区。
3. 从 Goerli Plaza 仓库中，将以下文件复制到你的工作区：
   * `dcl-workspace.json`
   * `package.json`
   * `.gitignore`
4. 在此文件夹内，为你想要处理的每个项目在根目录下添加一个文件夹。你可以拖入包含场景或智能穿戴设备的现有文件夹。对于新文件夹，运行 `npx @dcl/sdk-commands init` 在每个文件夹中运行，以创建一个 Decentraland 项目。

   > 注意：确保每个场景中的地块彼此不重叠。
5. 在工作区文件夹中执行以下命令，以创建所需文件：

   `npm run update-parcels && npm run sync && npm run test && npm run format`

你可以通过打开以下文件来确认哪些项目属于该工作区： `dcl-workspace.json` 文件并检查其中列出的路径在其 `文件夹` 数组中。

## 运行工作区

运行 `npm run start` 在工作区的根文件夹上执行。这会同时运行所有项目，可在一个预览窗口中查看。此预览的行为与预览单个场景时完全相同。

工作区中的任何智能穿戴设备都可以通过在背包中查找来试穿。

## 添加项目

创建工作区后，你可以通过编辑以下文件来添加其他项目： `dcl-workspace.json` 文件手动添加。向 `文件夹` 数组中添加你想要加入的文件夹的相对路径。例如 `my-other-example`.

你也可以使用绝对路径添加不在工作区文件夹内的项目。


> [!WARNING]
> **📔 注意**：该文件夹中必须已包含一个使用以下方式初始化的 Decentraland 项目 `npx @dcl/sdk-commands init`。它不能是一个空文件夹。


要添加或移除项目，请编辑 `dcl-workspace.json` 文件，以在其中包含工作区内每个项目的相对路径，位于 `文件夹` 数组中。

```json
{
	"folders": [
		{
			"path": "example-scene"
		},
		{
			"path": "example-scene2"
		}
	]
}
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/xiang-mu-lei-xing/workspaces.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
