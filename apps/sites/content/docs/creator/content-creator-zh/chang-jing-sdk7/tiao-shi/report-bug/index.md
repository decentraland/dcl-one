> 如何报告错误，以便其被审查并修复

# 报告错误

如果您在使用 Creator Hub 中的 Decentraland SDK 和 Scene Editor 时遇到任何 bug 或问题，请在我们的 GitHub 仓库中报告 [这里](https://github.com/decentraland/sdk/issues/new/choose).

## 在报告 bug 之前

在提交 bug 报告之前，请确保以下几点：

* 请确保您使用的是最新版本的 SDK，以防该问题已在更新的版本中修复。
* 请查看您尝试使用的功能的文档，以确认它的行为应当符合您的预期，并且这并不是误解。
* 排除问题可能是由您的场景构建方式导致的。请参见 [预览中调试](/docs/creator/content-creator-zh/chang-jing-sdk7/tiao-shi/debug-in-preview/) 和 [正式环境中调试](/docs/creator/content-creator-zh/chang-jing-sdk7/tiao-shi/debug-in-prod/) 获取提示。
* 避免重复。查看 [已知 bug 列表](https://github.com/orgs/decentraland/projects/20/views/13) 以避免重复提交。如果您有更多信息要补充到现有问题中，请在该问题下添加评论，而不是创建新的问题。

## 报告 bug

要报告 SDK 的 bug，您需要一个 GitHub 账号。请按以下步骤操作：

1. 访问以下链接： [新建问题页面](https://github.com/decentraland/sdk/issues/new/choose)
2. 点击 **开始** 位于 **Bug 报告**. ![](/docs/_remote/b51c88e272f5593a42575857.webp)
3. 请填写模板中的所有字段。模板为每个部分都提供了说明。您能提供的细节越多，我们的开发人员就能越快地识别并解决该问题。
4. 点击 **提交新问题**.

![](/docs/_remote/dc32a849c6f1a59a64d4565b.webp)

如果开发人员需要澄清或补充信息，可能会在您的工单中提出后续问题。您将通过电子邮件收到 bug 报告更新通知。


> [!WARNING]
> **📔 注意**：对于任何与 SDK 无关的问题，请联系支持团队。要联系我们，请访问此页面 [intercom.decentraland.org](https://intercom.decentraland.org/) 或者发送电子邮件至 <hello@decentraland.org>。您也可以在 [Discord](https://decentraland.org/discord/)上找到我们。团队可在周一至周日 UTC 时间中午 12 点到晚上 9 点提供帮助，并会尽力尽快回复。您可以预计在上述时间段内于 Discord 在 60 分钟内收到回复，电子邮件则在 24 小时内收到回复。
> 
> 您也可以联系 DAO 支持团队，方式是在 [DAO Discord](https://discord.gg/bxHtcMxUs4).


**bug 报告的其他提示：**

* 为每个 bug 创建单独的问题。如果您遇到多个问题，请确保为每一个都创建新问题。
* 避免冗长的说明。仅在必要时包含相关的命令行或控制台输出，或场景代码。
* 如果可能，请附上图片或视频。您可以轻松地将它们拖放到问题文本区域中，GitHub 会处理上传和链接。

## 使用 Decentraland Playground

提供能够展示问题的代码片段非常有用。可复现性至关重要。如果无法复现 bug，就很难诊断和修复。

您可以将代码片段粘贴到问题文本中，但提供代码片段的最佳方式是使用 [Decentraland Playground](https://playground.decentraland.org/)。请按以下步骤操作：

1. 在左侧编写您的代码片段，在右侧面板查看您的场景。
2. 点击 **分享** 按钮，并将链接粘贴到您的问题报告中。

![](/docs/_remote/14f730a1e519a0ed272964d1.webp)

使用 playground 可简化问题复现，并确保任何打开该链接的人都能看到相同的输出，而不受其操作系统、SDK 版本、Node 版本等影响。它还便于快速迭代，以识别受影响的条件并定位问题原因。

请尽量让您的示例简洁，同时仍能复现该问题。更简单的示例有助于消除对问题来源的歧义。


> [!WARNING]
> **📔 注意**：在 Playground 中，无法将资源导入到您的场景中。因此，如果复现该问题需要导入 3D 模型或音频文件，那么在那里将无法显示。


## 获取日志

在报告 bug 时，获取 Decentraland explorer 的日志通常很有帮助。

要获取日志，只需打开聊天并输入 `/logs`。这将打开一个新窗口，显示 Decentraland explorer 的日志。请将 `Player.log` 文件附加到您的问题报告中。

## 编辑文档

如果您在文档网站的内容中发现问题，可以提交修改请求。只需滚动到页面底部并点击 **编辑此页面** 按钮。GitHub 会引导您创建一个包含更改的拉取请求。基金会团队会审核您的更改，并在其被认为有用时发布它们。

## 功能请求和建议

如果您有添加新功能或特性的建议，而不是报告问题，请加入 [Decentraland Discord 服务器](https://dcl.gg/discord) 并在 [Creator Hub 频道](https://discord.com/channels/417796904760639509/1288888172318560326) 中发布消息，而不是创建 GitHub Issue。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/tiao-shi/report-bug.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
