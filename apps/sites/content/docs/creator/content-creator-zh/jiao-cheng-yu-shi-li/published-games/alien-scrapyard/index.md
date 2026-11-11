> 通过数据定义的建造谜题和由阶段驱动的电影镜头进行协作建造回合。

# 外星废料场

![外星废料场预览](/docs/_remote/c94532935e9e0629a121ed5f.webp)

一款合作式限时建造游戏，玩家共同组装外星科技并解锁新的计分和形状。

* [在 Decentraland 上游玩](https://decentraland.org/jump?realm=alienscrapyard.dcl.eth)

## 项目信息

**维护者：** 社区 · procesista

[查看上游源码](https://github.com/procesista/Alienscrap)

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

### 将建造谜题表示为数据

槽位定义描述了每个部件所需的零件、位置、缩放比例和标签。模板辅助函数会重新居中并缩放布局，公开一系列谜题，并查找单个槽位。这对于研究如何通过数据添加新的构造很有帮助。

**项目背景：** 请结合共享的场景中心和部件类型常量一并阅读。

**源码参考：**

* [槽位定义](https://github.com/procesista/Alienscrap/blob/be0b1b93580452f9cc778dc36554fe9354ee0ea2/src/shared/templates.ts#L3-L39)
* [模板辅助函数](https://github.com/procesista/Alienscrap/blob/be0b1b93580452f9cc778dc36554fe9354ee0ea2/src/shared/templates.ts#L140-L184)

### 将电影镜头相机绑定到回合状态

电影镜头系统会在倒计时和表演阶段激活镜头，处理玩家在回合中途加入的情况，并在快照过期时释放相机。最终更新在将相机沿其镜头移动之前，新增了回合切换和经过时间退出条件。

**项目背景：** 该示例依赖于客户端快照、共享的计时常量以及 HUD 可见性辅助函数。

**源码参考：**

* [电影镜头系统](https://github.com/procesista/Alienscrap/blob/be0b1b93580452f9cc778dc36554fe9354ee0ea2/src/client/cinematic.ts#L93-L129)
* [最终更新](https://github.com/procesista/Alienscrap/blob/be0b1b93580452f9cc778dc36554fe9354ee0ea2/src/client/cinematic.ts#L158-L176)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/jiao-cheng-yu-shi-li/published-games/alien-scrapyard.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
