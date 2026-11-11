> 具有限时回合、持久化排行榜、复用投射物以及基于距离的光照预算的多人躲避游戏。

# 夺旗游戏

![旗帜标签预览](/docs/_remote/a8bf7285858279143edb58ed.webp)

一款全天候多人服务器的“躲避抢夺”游戏。玩家在五分钟一局的回合中竞争持有旗帜，使用回旋镖和香蕉陷阱，同时持续记录每日、每月和历史总榜结果。

* [在 Decentraland 上游玩](https://decentraland.org/jump?realm=flagtag.dcl.eth)

## 项目信息

**维护者：** 社区 · iillee

[查看上游源码](https://github.com/iillee/flagtag)

**许可证：** 本目录中未标识。在重新使用前，请先查阅上游仓库和维护者以了解适用条款。


> [!WARNING]
> **社区维护代码——使用前请审查**
> 
> 该社区项目由 Decentraland 基金会和 DAO GitHub 组织之外的团队维护。其源代码和线上部署可能会独立变更。
> 
> 收录到本目录并不意味着已通过安全审计，也不保证安全。在运行或重新使用前，请审查代码、依赖项和安装脚本。
> 
> [报告目录问题](https://github.com/decentraland/docs/issues/new)。如怀疑存在漏洞，请遵循上游项目的安全策略。不要在公开 issue 中包含利用细节或凭据。


## 主要功能

* 多人服务器
* 用于存储玩家进度和解锁内容
* 用于自定义动画的头像遮罩

## 浏览代码

这些链接指向特定的源码快照，因此即使上游持续开发，相关说明仍然有用。它们是学习参考，而非安全评估。

### 按模型变体复用投射物视觉效果

投射物池会为每种回旋镖颜色预先创建十个隐藏实体。获取和释放辅助函数会查找可用实体，并在停止其声音后将其归还到隐藏位置。

**项目背景：** 飞行代码必须定位已获取的实体，并在结束时将其归还；该对象池依赖项目的回旋镖资源和声音辅助函数。

**源码参考：**

* [投射物池](https://github.com/iillee/flagtag/blob/d5d9c8fbf0f69eadb4d79c3ae4054e8947f256b6/src/systems/projectile/pool.ts#L11-L37)
* [获取和释放辅助函数](https://github.com/iillee/flagtag/blob/d5d9c8fbf0f69eadb4d79c3ae4054e8947f256b6/src/systems/projectile/pool.ts#L48-L66)

### 限制玩家周围的活动灯光

proximityLightSystem 会定期检查，使用平方距离比较和可复用数组选出最近的灯光，并且只更改活动状态不同的灯光。场景将活动灯光数量上限设为 8，并通过其昼夜辅助函数进行控制。

**项目背景：** 请阅读同一文件中的设置和常量，以了解世界坐标转换以及针对不同设备的强度设置。

**源码参考：**

* [proximityLightSystem](https://github.com/iillee/flagtag/blob/d5d9c8fbf0f69eadb4d79c3ae4054e8947f256b6/src/systems/proximityLights.ts#L109-L168)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/jiao-cheng-yu-shi-li/published-games/flag-tag.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
