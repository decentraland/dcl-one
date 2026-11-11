> 由 Google Sheet 驱动的场景内容，无需重新部署即可更新。

# 来自电子表格的数据

从 Google 表格驱动场景内容。编辑表格即可更改艺术家名称、视频流和展示的可穿戴物品，而无需重新部署场景。

## 项目信息

**维护者：** Decentraland

[查看源代码](https://github.com/decentraland/Read_Spreadsheet_Scene)

**许可证：** [Apache-2.0](https://github.com/decentraland/Read_Spreadsheet_Scene/blob/main/LICENSE)。在重复使用前请检查上游条款；代码和资源可能适用不同条款。

## 浏览代码

这些链接指向特定的源码快照，因此即使上游持续开发，相关说明仍然有用。它们是学习参考，而非安全评估。

### 将已发布的 Google 表格读取为 JSON

`getSpreadsheetData` 通过 Google Visualization 端点请求该表格（`gviz/tq?tqx=out:json`），去除 `setResponse(...)` 包装器，并将每一行转换为一个以列标题为键的对象。

**项目背景：** 该表格必须设置为共享，这样拥有链接的任何人都可以查看。 `gid` 用于选择工作表标签页并 `headers=1` 将第一行视为列名。

**源码参考：**

* [`getSpreadsheetData`](https://github.com/decentraland/Read_Spreadsheet_Scene/blob/57bb4086aed3054d69a74f33f20f2d86c4eb9bb6/src/scheduler.ts#L1-L47)

### 将一行应用到场景

`updateFromSpreadsheet` 读取一行并更新横幅 `TextShape`，替换屏幕的 `VideoPlayer` 资源，切换展示在 dispenser 上的可穿戴物品，并将 dispenser 的点击动作指向该行的购买链接。

**项目背景：** 诸如以下的列名： `NowPlaying`, `VideoURL`, `WearableURN`，以及 `BuyURL` 必须与表格标题匹配。实体会根据 Creator Hub 场景中给定的名称进行查找。

**源码参考：**

* [`updateFromSpreadsheet`](https://github.com/decentraland/Read_Spreadsheet_Scene/blob/57bb4086aed3054d69a74f33f20f2d86c4eb9bb6/src/index.ts#L10-L64)
* [`createWearable`](https://github.com/decentraland/Read_Spreadsheet_Scene/blob/57bb4086aed3054d69a74f33f20f2d86c4eb9bb6/src/index.ts#L100-L130)

### 通过 Creator Hub 按钮循环浏览各行

`main` 监听 `Next` 和 `Previous` 在以下对象上的动作： `Switch Artist` 实体，并在下载的数据范围内循环行索引。

**项目背景：** 使用 `getActionEvents` 来自 `@dcl/asset-packs`，因此这些动作是在 Creator Hub 中的实体上配置的。

**源码参考：**

* [`main`](https://github.com/decentraland/Read_Spreadsheet_Scene/blob/57bb4086aed3054d69a74f33f20f2d86c4eb9bb6/src/index.ts#L69-L97)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/jiao-cheng-yu-shi-li/external-services/spreadsheet-data.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
