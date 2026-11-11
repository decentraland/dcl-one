> 如何向场景添加库

# 管理依赖项

很多时候，你的场景可能需要一些已经封装在可复用库中的功能。使用外部库可以让你的工作轻松得多。

你可以将外部库导入到 Decentraland 项目中，然后引用这些库中描述的系统、组件或函数。

查看 [示例页面](https://studios.decentraland.org/resources?sdk_version=SDK7&resource_type=Library) 以找到一系列由 Decentraland 基金会和社区成员创建的、可解决常见问题的库。

## 通过 CLI

### 安装

要在项目文件夹中安装一个库，请运行 `npm i` 以及你项目根路径中的库名称。例如：

`npm i @dcl-sdk/utils`

> **📔 注意**：你可能需要运行 `npm run start` 或 `npm run build` 以便在安装库后安装该库的依赖项。

### 更新

如果你的 `package.json` 文件将该库的版本列为 `@latest`，那么运行 `npm i` 以将所有库更新到各自的最新版本。

如果你的 `package.json` 引用的是一个特定版本号，你可以通过运行安装该库的命令来更新它，并在末尾添加 `@` 和版本号。例如：

`npm i @dcl-sdk/utils@1.7.5`

### 卸载

要从场景的依赖中删除一个库，请运行 `npm rm` 以及库名称。例如：

`npm rm @dcl-sdk/utils`

删除任何你不使用的库是个好习惯。未使用的库仍会占用场景空间，影响玩家下载和运行你的场景时的体验。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/ku/manage-dependencies.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
