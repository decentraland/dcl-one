> 通过共享组件轮询和附着在头像上的道具来反映携带物品的合作式夜店清理。

# 清理俱乐部

![《清理俱乐部》预览](/docs/_remote/a8f103a70354f2dfa74d73d7.webp)

一款合作清理游戏，玩家要在下一场派对开始前争先恢复夜总会。

* [在 Decentraland 上游玩](https://decentraland.org/jump?realm=cleantheclub.dcl.eth)

## 项目信息

**维护者：** 社区 · CleanTheClub

[查看上游源代码](https://github.com/CleanTheClub/CleanTheClub)

**许可证：** [Apache-2.0](https://github.com/CleanTheClub/CleanTheClub/blob/main/LICENSE)。重复使用前请检查上游条款；代码和资源可能适用不同条款。


> [!WARNING]
> **社区维护的代码——使用前请审查**
> 
> 此社区项目由 Decentraland Foundation 和 DAO 的 GitHub 组织之外的团队维护。其源代码和线上部署可能会独立变更。
> 
> 收录于此目录并不意味着已进行安全审计或保证其安全性。在运行或重复使用之前，请审查代码、依赖项和安装脚本。
> 
> [报告目录问题](https://github.com/decentraland/docs/issues/new)。对于疑似漏洞，请遵循上游项目的安全政策。不要在公开 issue 中包含利用细节或凭据。


## 探索代码

这些链接指向特定的源代码快照，因此即使上游持续开发，这些说明仍然有用。它们是学习参考，不是安全评估。

### 在多个系统间共享一次组件扫描

onClutterPoll 会在每个轮询间隔收集一次 ClutterSync 条目，构建一个物品 ID 查找表，并将同一快照传递给订阅者。每个消费者都可以自行进行过滤和变更检测。

**项目上下文：** 这有助于研究如何避免重复的 ECS 扫描；它依赖于共享的 schema 和阶段门轮询间隔，而订阅应放在客户端初始化中。

**源代码参考：**

* [onClutterPoll](https://github.com/CleanTheClub/CleanTheClub/blob/8ffc6c1fa24ff0da22a1d82ecdf2f9ce72f47f73/src/client/clutterWatcher.ts#L15-L54)

### 显示另一名玩家携带的装备

updateRemoteCarry 会将一个道具层级附加到另一名化身的左手，从当前装备或搬运状态中选择一个模型，并根据袋子的填充比例进行缩放。若没有可显示内容，它也会移除该层级。

**项目上下文：** 请结合携带消息处理器、主题模型查找和模型适配偏移一起阅读：这些提供了状态和特定资源的定位信息。

**源代码参考：**

* [updateRemoteCarry](https://github.com/CleanTheClub/CleanTheClub/blob/8ffc6c1fa24ff0da22a1d82ecdf2f9ce72f47f73/src/client/carrySystem.ts#L805-L846)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/jiao-cheng-yu-shi-li/published-games/clean-the-club.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
