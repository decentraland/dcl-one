> 一个多人节日小径场景，包含自定义头像、Marketplace 集成、由射线检测驱动的色轮和跟踪摄像机。

# 登阶斩击

![Slay the Steps 预览](/docs/_remote/009ced18986123a766b6a4fb.webp)

一个由 Studio 制作的多人服务器体验，专为 Decentraland Festive Trail 2025 打造。它包含自定义头像、Marketplace API 集成以及自定义颜色轮选择器。其代码将客户端、服务器和共享模块分离，并将源资产与可部署场景一并提供。

* [在 Decentraland 上游玩](https://decentraland.org/jump?realm=slaythesteps.dcl.eth)

## 项目信息

**维护者：** 社区 · stom66

[查看上游源码](https://github.com/stom66/dcl-slay-the-steps)

**许可证：** [CC BY-NC-ND 4.0](https://github.com/stom66/dcl-slay-the-steps/blob/main/LICENSE.md)。在重复使用前请检查上游条款；代码和资源可能适用不同条款。


> [!WARNING]
> **社区维护代码——使用前请审查**
> 
> 该社区项目由 Decentraland 基金会和 DAO GitHub 组织之外的团队维护。其源代码和线上部署可能会独立变更。
> 
> 收录到本目录并不意味着已通过安全审计，也不保证安全。在运行或重新使用前，请审查代码、依赖项和安装脚本。
> 
> [报告目录问题](https://github.com/decentraland/docs/issues/new)。如怀疑存在漏洞，请遵循上游项目的安全策略。不要在公开 issue 中包含利用细节或凭据。


## 主要功能

* 电影感 **虚拟摄像机**
* **AvatarShapes** 用于展示可穿戴物品
* **多人** 用于同步回合/玩家装扮
* 自定义 3D 颜色选择器

## 浏览代码

这些链接指向特定的源码快照，因此即使上游持续开发，相关说明仍然有用。它们是学习参考，而非安全评估。

### 将 3D 指针点击转换为颜色选择

`GetLocalMousePosition` 从摄像机发射射线，并将命中点转换为色轮的局部位置。 `SampleColorWheel` 将其角度和半径映射为色相和饱和度，调用回调，并为明度滑块着色。

**项目背景：** 该实现使用了项目的色轮网格、HSV 转换辅助工具和材质节点名称。

**源码参考：**

* [`GetLocalMousePosition`](https://github.com/stom66/dcl-slay-the-steps/blob/9f6026c826fb61d25eeea6a98f22a4dd16b29a15/dcl/src/client/colorPickers.ts#L306-L335)
* [`SampleColorWheel`](https://github.com/stom66/dcl-slay-the-steps/blob/9f6026c826fb61d25eeea6a98f22a4dd16b29a15/dcl/src/client/colorPickers.ts#L203-L238)

### 使用虚拟摄像机跟踪移动目标

摄像机位置系统和补间链会定期计算相对于玩家和目标的位置，然后补间移动过去。 `TrackEntity` 使用……创建摄像机 `lookAtEntity` 并使其激活。

**项目背景：** 读取 `ResetCamera` 以及游戏在跟踪生命周期结束时的事件绑定。

**源码参考：**

* [摄像机位置系统和补间链](https://github.com/stom66/dcl-slay-the-steps/blob/9f6026c826fb61d25eeea6a98f22a4dd16b29a15/dcl/src/client/cameraController.ts#L46-L96)
* [`TrackEntity`](https://github.com/stom66/dcl-slay-the-steps/blob/9f6026c826fb61d25eeea6a98f22a4dd16b29a15/dcl/src/client/cameraController.ts#L99-L137)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/jiao-cheng-yu-shi-li/published-games/slay-the-steps.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
