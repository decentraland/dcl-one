> 具有服务器托管游戏状态、持久化进度、炮塔摄像机和屏幕边缘目标标记的合作飞船战斗。

# 星际船员

![Astro Crew 预览](/docs/_remote/31ce14ffd2ff2d42c05fb6b7.webp)

一个合作场景，船员共用一艘飞船。每到一站，他们会射击来袭的小行星和飞碟，修复船体破损，并提升炮手和工程等级。战斗运行在权威式多人服务器上。

* [在 Decentraland 上游玩](https://decentraland.org/play/?realm=astrocrew.dcl.eth)

## 项目信息

**维护者：** 社区 · DansStuff

[查看上游源码](https://github.com/DansStuff/AstroCrew)

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

* **权威式多人联机** 整场战斗期间。客户端发送瞄准、修复和过载请求。服务器负责管理船体、破损和生成点，并同步 `GameState` 和 `PlayerStats` 只接受来自多人服务器的写入。
* **存储** 为每位玩家的炮手和工程等级，以及每周任务排行榜。
* **虚拟摄像机** 在三个炮塔上，并通过定时过渡混合呈现。撞击会让这些摄像机震动。拿起武器会将化身冻结，并带有 **InputModifier** 并使用 **TouchScreenControls**.
* **PrimaryPointerInfo** 根据指针的世界射线来瞄准每一发射击。 **PointerLock** 保持释放状态，以便 **PointerEvents** 仍然可以控制控制台、按住修复破口，以及任务表。
* **Gltf 节点修饰器** 重设飞船自发光网格的颜色：巡航时为绿色，遭遇战时为红色，而武器过载时使用另一套颜色。
* **ReactEcs** 用于显示船体、技能等级和回合结束面板的 HUD。屏幕外的破口图标位于屏幕边缘，使用 **UiCanvasInformation**.
* **Billboards**, **TextShape**，以及 **avatar textures** 在每个目标上：一个朝向摄像机的准星，一个“Target Locked”标签，以及正在射击它的人的头像。
* **Animator** 循环播放飞碟旋转片段。 **AudioSource** 播放短音效和循环音效，并配有激光一次性音效池，因此当上一段音频仍在播放时，射击音效也能重新开始。音频片段通过 **AssetLoad**.
* 游戏世界内 **TextShape** 周排行榜的行。点击某一行可打开该任务的结果。
* **AvatarModifierArea** 关闭整个场景中的通行证提示。

## 浏览代码

这些链接指向特定的源码快照，因此即使上游持续开发，相关说明仍然有用。它们是学习参考，而非安全评估。

### 引导玩家前往屏幕外目标

edgePositionForBreach 将破口方向投影到摄像机的水平轴上，找到矩形边缘，并使用画布宽高比将图标限制在屏幕内。BreachIcon 将该位置连接到一个脉动的 ReactEcs 标记。

**项目背景：** 结合场景中的破口位置、活动破口状态和 UI 常量读取它。

**源码参考：**

* [edgePositionForBreach](https://github.com/DansStuff/AstroCrew/blob/dbe11b05be2a753b27b6d5dd866d0554ff73344f/src/ui/breachIcons.tsx#L27-L76)
* [BreachIcon](https://github.com/DansStuff/AstroCrew/blob/dbe11b05be2a753b27b6d5dd866d0554ff73344f/src/ui/breachIcons.tsx#L78-L107)

### 添加会回到原始姿态的摄像机震动

shakeEntity 记录一个基础变换，并使用强度和剩余持续时间合并重叠的震动。CameraShakeSystem 施加逐渐减弱的位置和旋转偏移，然后恢复基础姿态并移除该组件。

**项目背景：** 该效果使用项目的振幅常量，并且必须注册为 ECS 系统。

**源码参考：**

* [shakeEntity](https://github.com/DansStuff/AstroCrew/blob/dbe11b05be2a753b27b6d5dd866d0554ff73344f/src/effects/cameraShake.ts#L13-L47)
* [CameraShakeSystem](https://github.com/DansStuff/AstroCrew/blob/dbe11b05be2a753b27b6d5dd866d0554ff73344f/src/effects/cameraShake.ts#L87-L121)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/jiao-cheng-yu-shi-li/published-games/astro-crew.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
