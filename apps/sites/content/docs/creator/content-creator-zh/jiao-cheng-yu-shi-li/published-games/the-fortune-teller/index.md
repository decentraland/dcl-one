> 社交占卜，带有明确的对话状态转换和基于支点的电影镜头装置。

# 占卜师

![《算命师》预览](/docs/_remote/2aaa323560ef094922b15d6a.webp)

一种社交角色扮演体验，玩家可以为朋友占卜，或让朋友为自己占卜。

* [在 Decentraland 上游玩](https://decentraland.org/jump?realm=fortunes.dcl.eth)

## 项目信息

**维护者：** 社区 · carlosmu

[查看上游源码](https://github.com/carlosmu/circus_fortune_teller)

**许可证：** 本目录中未标识。在重新使用前，请先查阅上游仓库和维护者以了解适用条款。


> [!WARNING]
> **社区维护代码——使用前请审查**
> 
> 该社区项目由 Decentraland 基金会和 DAO GitHub 组织之外的团队维护。其源代码和线上部署可能会独立变更。
> 
> 收录到本目录并不意味着已通过安全审计，也不保证安全。在运行或重新使用前，请审查代码、依赖项和安装脚本。
> 
> [报告目录问题](https://github.com/decentraland/docs/issues/new)。如怀疑存在漏洞，请遵循上游项目的安全策略。不要在公开 issue 中包含利用细节或凭据。


## 浏览代码

这些链接指向特定的源码快照，因此即使上游持续开发，相关说明仍然有用。它们是学习参考，而非安全评估。

### 将一次解读表示为显式状态转换

算命状态机列出了从类别和牌组选择到揭示结果的允许步骤。 `tryTransition` 检查下一步，更新会话，并通知状态进入监听器。

**项目背景：** 结合会话类型和动作处理器阅读它，看看多步骤交互是如何组织的。

**源码参考：**

* [算命状态机](https://github.com/carlosmu/circus_fortune_teller/blob/f8db0c5630a03208d328a96737224e1293df60c7/src/fortuneFsm/machine.ts#L6-L60)

### 围绕场景对象构建一个摄影机绑定装置

`ensureHostPivotRig` 和 `startHostCinematicCamera` 将一个虚拟摄像机挂到桌子下方的一个枢轴点上，从玩家一侧选择起始和结束角度，并激活摄像机。

**项目背景：** 该绑定装置依赖于桌子实体、本模块中的配置值和更新系统；这些部分说明了如何在其场景上下文中构建一次交互画面。

**源码参考：**

* [`ensureHostPivotRig` 和 `startHostCinematicCamera`](https://github.com/carlosmu/circus_fortune_teller/blob/f8db0c5630a03208d328a96737224e1293df60c7/src/cinematicCamera.ts#L141-L197)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/jiao-cheng-yu-shi-li/published-games/the-fortune-teller.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
