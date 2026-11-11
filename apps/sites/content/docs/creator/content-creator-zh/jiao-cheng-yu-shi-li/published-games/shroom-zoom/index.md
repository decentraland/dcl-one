> 平台跳跃玩法，带有由触发器激活的弹跳射线检测、玩家冲量，以及会跟随或环绕其拥有者的可收集精灵图。

# 蘑菇冲刺

![Shroom Zoom 预览](/docs/_remote/abaf092d80ad05ebe9c8fc1b.webp)

一款平台跳跃和计时竞速游戏，包含二段跳、滑翔、收集物和探索。

* [在 Decentraland 上游玩](https://decentraland.org/jump?realm=shroomzoom.dcl.eth)

## 项目信息

**维护者：** 社区 · CanessaDCL

[查看上游源码](https://github.com/CanessaDCL/dcl-shroomzoom)

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
* 为弹跳蘑菇设计的物理冲量

## 浏览代码

这些链接指向特定的源码快照，因此即使上游持续开发，相关说明仍然有用。它们是学习参考，而非安全评估。

### 将附近触发器与精确接触检测结合起来

BounceTrigger 回调在玩家处于触发器内时启用射线检测，并在离开时移除该系统。射线检测和弹跳逻辑会检查向下命中预设的蘑菇实体，并在其表面附近向玩家施加冲量。

**项目背景：** 请与 BounceZoom 或 BounceRegular 一起阅读，以了解该场景的触发器尺寸和冲量设置。

**源码参考：**

* [BounceTrigger 回调](https://github.com/CanessaDCL/dcl-shroomzoom/blob/042968e3c13b5d1093d2922d5713570ad4cb0284/src/bounce/class.bounce.ts#L75-L101)
* [射线检测和弹跳逻辑](https://github.com/CanessaDCL/dcl-shroomzoom/blob/042968e3c13b5d1093d2922d5713570ad4cb0284/src/bounce/class.bounce.ts#L103-L164)

### 将收集到的物品变成一个会动的随行队伍

跟随目标辅助函数会计算玩家周围和轨道上的偏移位置。followSpritesSystem 会将每个精灵插值移动到其目标位置，在收集完成后切换为环绕轨道，并让精灵朝向其拥有者旋转。

**项目背景：** 该场景的收集集合、跟随实体以及每位玩家的轨道时间戳为这种视觉行为提供状态。

**源码参考：**

* [跟随目标辅助函数](https://github.com/CanessaDCL/dcl-shroomzoom/blob/042968e3c13b5d1093d2922d5713570ad4cb0284/src/spriteFollowers.ts#L400-L437)
* [followSpritesSystem](https://github.com/CanessaDCL/dcl-shroomzoom/blob/042968e3c13b5d1093d2922d5713570ad4cb0284/src/spriteFollowers.ts#L678-L706)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/jiao-cheng-yu-shi-li/published-games/shroom-zoom.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
