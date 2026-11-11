> 一个协作花园，具有持久化植物状态、服务器时钟偏移估算和复用的花瓣动画。

# 花开花园

![Bloom Garden 预览](/docs/_remote/3a9e2127ba7a6f3cdcd435b3.webp)

一种合作式花园体验，玩家共同维护一个生机勃勃的花园，并唤醒大绽放。

它的多人服务器使用内置存储保存植物状态，设有每周排行榜，以及每位玩家的浇水次数限制。

* [在 Decentraland 上游玩](https://decentraland.org/jump?realm=bloomgarden.dcl.eth)

## 项目信息

**维护者：** 社区 · TheLivingGarden

[查看上游源码](https://github.com/TheLivingGarden/TheLivingGarden/tree/main-main)

**许可证：** [MIT](https://github.com/TheLivingGarden/TheLivingGarden/blob/main/LICENSE)。在重复使用前请检查上游条款；代码和资源可能适用不同条款。


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

### 估算服务器与客户端时钟之间的偏移量

时钟辅助器会保留五个偏移样本，拒绝较大的偏差，并将服务器时间戳转换为本地时间。浇水客户端会把通过场景消息定义接收到的时间戳传给它。

**项目背景：** 这演示了一个轻量级的时间估算；网络传输时间已包含在这些样本中。

**源码参考：**

* [时钟辅助器](https://github.com/TheLivingGarden/TheLivingGarden/blob/b2c9a90791b1eed5c92a4ffadf6ac480d91510ea/src/shared/clockSync.ts#L3-L55)
* [浇水客户端](https://github.com/TheLivingGarden/TheLivingGarden/blob/b2c9a90791b1eed5c92a4ffadf6ac480d91510ea/src/wateringSystem.ts#L1144-L1146)

### 让临时效果自然结束

花瓣设置和控制会根据场景中名为 Petal 的模型创建一个可重复使用的对象池，然后启动或收尾该效果。更新阶段会将花瓣缩放进来，让它们落下并停留，然后在回收前缩小它们。当某个效果应停止生成，而现有对象需要完成动画时，请研究这一点。

**项目背景：** 放置位置取决于 BLOOM\_CENTER 和模块的时间常量。

**源码参考：**

* [花瓣设置和控制](https://github.com/TheLivingGarden/TheLivingGarden/blob/b2c9a90791b1eed5c92a4ffadf6ac480d91510ea/src/petalSystem.ts#L119-L173)
* [更新阶段](https://github.com/TheLivingGarden/TheLivingGarden/blob/b2c9a90791b1eed5c92a4ffadf6ac480d91510ea/src/petalSystem.ts#L207-L310)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/jiao-cheng-yu-shi-li/published-games/bloom-garden.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
