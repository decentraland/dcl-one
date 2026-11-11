> 在变化的塔层中进行限时平台跳跃，并带有资源预加载和可跳过的关卡镜头导览。

# 疯狂之塔

![疯狂之塔预览](/docs/_remote/ec24d5b235cc62f71cd1eae3.webp)

一款计时平台跳跃游戏，玩家竞速冲向塔顶。

* [在 Decentraland 上游玩](https://decentraland.org/jump?realm=towerofmadness.dcl.eth)

## 项目信息

**维护者：** Decentraland DAO

[查看源代码](https://github.com/dcl-regenesislabs/towerofmadness)

**许可证：** 本目录中未标识。在重新使用前，请先查阅上游仓库和维护者以了解适用条款。

## 浏览代码

这些链接指向特定的源码快照，因此即使上游持续开发，相关说明仍然有用。它们是学习参考，而非安全评估。

### 预加载可变关卡的各个部分

`setupChunkPreload` 去重分块资源路径并创建临时 GLTF 实体。一个系统监视它们的加载状态，在加载完成或失败后移除实体，并在没有剩余实体时注销自身。

**项目背景：** 这展示了如何管理预加载生命周期；资源列表来自 `shared/chunks` 并且是该塔专属的。

**源码参考：**

* [`setupChunkPreload`](https://github.com/dcl-regenesislabs/towerofmadness/blob/be318c6f0a287c15f3771f12f629b84de6cc18af/src/chunkPreload.ts#L22-L61)

### 围绕不断变化的几何体构建一个可跳过的镜头导览

`playCinematic` 围绕当前的起始和结束分块调整摄像机关键帧，然后选择一个虚拟摄像机。更新系统会插值位置和注视目标，并支持按住按钮跳过。

**项目背景：** 关注分块查找辅助函数以及 `skipCinematic` 以了解导览如何找到其锚点并将控制权返回给玩家摄像机。

**源码参考：**

* [`playCinematic`](https://github.com/dcl-regenesislabs/towerofmadness/blob/be318c6f0a287c15f3771f12f629b84de6cc18af/src/cinematicCamera.ts#L110-L209)
* [更新系统](https://github.com/dcl-regenesislabs/towerofmadness/blob/be318c6f0a287c15f3771f12f629b84de6cc18af/src/cinematicCamera.ts#L286-L334)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/jiao-cheng-yu-shi-li/published-games/tower-of-madness.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
