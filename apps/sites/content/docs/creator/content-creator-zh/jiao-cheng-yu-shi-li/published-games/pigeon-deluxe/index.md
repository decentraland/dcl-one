> 限时包裹投递，包含取件和投递区域、安装在头像上的方向指示器，以及与投递动画分离的计分。

# 豪华鸽子

![Pigeon Deluxe 预览](/docs/_remote/5140367a547cfe389db77f7e.webp)

一款限时投递游戏，玩家收集包裹，将其送到另一个平台，并通过投递获得额外时间。

* [在 Decentraland 上游玩](https://decentraland.org/jump?realm=pigeondeluxe.dcl.eth)

## 项目信息

**维护者：** 社区 · PepeGawd

[查看上游源代码](https://github.com/PepeGawd/Pigeon-Deluxe)

**许可：** 未在此目录中标识。复用前请查阅上游仓库和维护者以了解适用条款。


> [!WARNING]
> **社区维护的代码——使用前请审查**
> 
> 此社区项目在 Decentraland Foundation 和 DAO 的 GitHub 组织之外维护。其源代码和在线部署可能会独立更改。
> 
> 收录于此目录并不意味着经过安全审计或保证安全。运行或复用前，请审查代码、依赖项和安装脚本。
> 
> [报告目录问题](https://github.com/decentraland/docs/issues/new)。对于疑似漏洞，请遵循上游项目的安全政策。不要在公开问题中包含利用细节或凭据。


## 探索代码

这些链接指向特定的源码快照，因此即使上游持续开发，这些说明仍然有用。它们是学习参考，不是安全评估。

### 附着在头像上时指向目标

`initParcelDirectionPointer` 及其更新系统会在头像姓名牌上方附加一个箭头，计算到当前目标的世界空间方向，并将该旋转转换为父对象的局部空间。旋转通过以下方式推进： `dt`，因此箭头会随着时间转向目标。

**项目背景：** 目标来自包裹系统，而箭头使用场景的 `FaceDirection1.glb` 资源。

**源代码参考：**

* [`initParcelDirectionPointer` 及其更新系统](https://github.com/PepeGawd/Pigeon-Deluxe/blob/1c24bcb3c82d73dbb1aad8b1035f3f7edab5d315/src/Game/parcelDirectionPointer.ts#L26-L96)

### 将计分与投递动画分离

`startNewRound` 和 `handleSpawnAreaEnter` 为平台分配取件和投递角色，跟踪包裹是否正在被携带，并在进入投递平台时立即奖励分数和时间。下一轮会在投放动画结束时开始。

**项目背景：** 结合场景的游戏计时器和包裹附着辅助工具一起阅读，看看游戏状态和视觉反馈如何协同配合。

**源代码参考：**

* [`startNewRound` 和 `handleSpawnAreaEnter`](https://github.com/PepeGawd/Pigeon-Deluxe/blob/1c24bcb3c82d73dbb1aad8b1035f3f7edab5d315/src/Game/parcelCollectDeliver.ts#L363-L433)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/jiao-cheng-yu-shi-li/published-games/pigeon-deluxe.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
