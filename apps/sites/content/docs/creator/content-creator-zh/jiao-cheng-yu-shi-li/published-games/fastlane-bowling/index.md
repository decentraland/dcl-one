> 采用服务器模拟物理、持久化进度、批量状态更新和适配移动端粒子效果的多人保龄球。

# 快线保龄球

![Fastlane Bowling 预览](/docs/_remote/926145e7ae23468dd849d344.webp)

一款由 Cannon-ES 提供逼真物理效果的多人保龄球游戏。多个游戏在共享空间中的独立球道上同时运行。

* [在 Decentraland 上游玩](https://decentraland.org/jump?realm=stom.dcl.eth)

## 项目信息

**维护者：** 社区 · stom66

[查看上游源码](https://github.com/stom66/dcl-bowling)

**许可证：** [CC BY-NC-ND 4.0](https://github.com/stom66/dcl-bowling/blob/main/LICENSE.md)。在重复使用前请检查上游条款；代码和资源可能适用不同条款。


> [!WARNING]
> **社区维护代码——使用前请审查**
> 
> 该社区项目由 Decentraland 基金会和 DAO GitHub 组织之外的团队维护。其源代码和线上部署可能会独立变更。
> 
> 收录到本目录并不意味着已通过安全审计，也不保证安全。在运行或重新使用前，请审查代码、依赖项和安装脚本。
> 
> [报告目录问题](https://github.com/decentraland/docs/issues/new)。如怀疑存在漏洞，请遵循上游项目的安全策略。不要在公开 issue 中包含利用细节或凭据。


## 主要功能

* [**Cannon-ES**](https://github.com/pmndrs/cannon-es) **物理** 在多人服务器上，客户端回放压缩后的关键帧。
* **存储** 用于玩家进度、统计、偏好、物品栏以及每周/历史排行榜。
* 动态 **灯光** 带有旋转阴影遮罩和跟随球体的聚光灯。
* **粒子** 轨迹和解锁特效，在移动设备上减少粒子数量。
* **虚拟摄像机** 用于到达飞入、瞄准和滚球回放。
* **`Gltf 节点修饰器`** 用于滚动霓虹纹理 UV 坐标。
* 由 SDK 控制的 **glTF 动画** 用于球道挡板、倒计时和计分特效。

## 浏览代码

这些链接指向特定的源码快照，因此即使上游持续开发，相关说明仍然有用。它们是学习参考，而非安全评估。

### 将同步状态更改批处理为客户端事件

在 `LaneWatcher.onLaneChanged`，一个集合收集已更改的球道并安排一次刷新。 `刷新` 读取每个球道的快照并发出状态和阶段转换事件。按照这个流程了解场景如何将同步组件连接到客户端响应。

**项目背景：** 请结合该项目的 `LaneStore` 以及事件总线定义。

**源码参考：**

* [`LaneWatcher.onLaneChanged`](https://github.com/stom66/dcl-bowling/blob/25c5a7f9152835c190d9d20fa18b6010d0c6ce50/dcl/src/client/laneWatcher.ts#L79-L90)
* [`刷新`](https://github.com/stom66/dcl-bowling/blob/25c5a7f9152835c190d9d20fa18b6010d0c6ce50/dcl/src/client/laneWatcher.ts#L132-L153)

### 为移动设备缩放粒子特效

`getParticleRatio` 和 `scaleConfig` 使用移动端比例调整发射速率、粒子上限和爆发次数，同时保留原始配置。 `生成` 将该配置应用于新的发射器，并可选地设置父级和定时清理。

**项目背景：** 这在研究一种特效配置如何适配不同设备时很有用。

**源码参考：**

* [`getParticleRatio` 和 `scaleConfig`](https://github.com/stom66/dcl-bowling/blob/25c5a7f9152835c190d9d20fa18b6010d0c6ce50/dcl/src/client/particleSpawner.ts#L176-L218)
* [`生成`](https://github.com/stom66/dcl-bowling/blob/25c5a7f9152835c190d9d20fa18b6010d0c6ce50/dcl/src/client/particleSpawner.ts#L91-L121)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/jiao-cheng-yu-shi-li/published-games/fastlane-bowling.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
