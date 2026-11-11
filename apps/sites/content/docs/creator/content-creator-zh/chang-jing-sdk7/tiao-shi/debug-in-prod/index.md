> 如何调试运行在 Decentraland 内的场景

# 生产环境调试

当运行一个已部署到 Decentraland 的场景时，你可以尝试多种方法来调试它。

## 在部署之前

### 预览

在将你的场景部署到 Decentraland 之前，请确保使用最新版本的 Decentraland SDK 在预览中正常运行。请参见 [在预览中调试](/docs/creator/content-creator-zh/chang-jing-sdk7/tiao-shi/debug-in-preview/).

## 快速重新加载

如果你需要重新加载你当前站立的场景，请在聊天中输入以下内容并按回车：

`/reload`

## 关于发布流程

请记住，每次发布后，系统都会先通过一个内部流程优化所有 3D 模型，然后才能渲染。这通常只需几秒钟，但对于非常大的场景或服务器繁忙时可能会更久。在完成之前，玩家看到的仍是你场景的上一版本。

你可以使用 [此工具](https://decentraland.github.io/opscli/)检查你场景中此过程的当前状态。如果转换已完成，资源的三种变体都应显示绿灯。

## 多人服务器日志和指标

对于已部署的 [多人服务器](/docs/creator/content-creator-zh/chang-jing-sdk7/lian-wang/authoritative-servers/) 场景，请使用 [场景洞察](/docs/creator/content-creator-zh/chang-jing-sdk7/tiao-shi/scene-insights/) 来搜索过去 7 天的服务器日志、实时跟踪新日志行，并查看 CPU 和内存指标。它还包括钱包登录以及你或你的 AI 助手可以从场景项目中运行的命令。

## 场景日志

在正常使用 Decentraland 时，无法打开控制台查看调试消息。要让控制台可用，你必须使用 `scene-console` 参数启动 decentraland。然后你可以通过按键盘上的反引号键来切换控制台： **\`**。在大多数英文键盘上，此键位于 1 键左侧。

要使用 `scene-console` 参数打开 Decentraland，请选择以下任一方式：

* 在浏览器窗口中输入以下深度链接： `decentraland://?position=0,0&scene-console=true`。如果你已安装，它将打开 Decentraland 桌面应用程序。
* 在命令行中输入以下内容：
  * **macOS**: `open Decentraland.app --args --position 0,0 --scene-console true`
  * **Windows**: `"C:\Users\[YOUR-USER]\Downloads\Decentraland_windows64\Decentraland.exe" --position 0,0 --scene-console true`


> [!NOTE]
> **💡 提示**: 更改 **position** 参数为你场景的坐标，即可直接加载到你的场景中。


在使用 `scene-console`运行 Decentraland 时，你可以通过三种方式打开控制台：

* 点击 ![](/docs/_remote/92e8eb247fcf289c2feff8f8.webp) 右上角的图标
* 按下 **\`** 该键可打开简短控制台
* 按 Shift + **\`** 以打开更大的控制台视图

请记住，每个活动场景中的消息都会记录到控制台，因此你在控制台中看到的某些内容可能与你的场景无关。

## 报告错误

如果你遇到的问题不是出在你的场景上，而是出在 Decentraland SDK 本身，请参见 [报告错误](/docs/creator/content-creator-zh/chang-jing-sdk7/tiao-shi/report-bug/).


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/tiao-shi/debug-in-prod.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
