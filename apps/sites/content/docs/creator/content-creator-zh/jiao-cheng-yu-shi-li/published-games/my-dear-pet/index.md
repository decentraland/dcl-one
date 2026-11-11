> 宠物收集玩法，移动带有加速与刹车效果，并对编辑器放置的蛋进行相位偏移动画。

# 我亲爱的宠物

![My Dear Pet 预览](/docs/_remote/3412460113d21fec792e322a.webp)

一款收集宠物的游戏。

* [在 Decentraland 上游玩](https://decentraland.org/jump?realm=marshcolony.dcl.eth)

## 项目信息

**维护者：** Decentraland DAO

[查看源代码](https://github.com/dcl-regenesislabs/marsh-colony)

**许可证：** 在此目录中未标识。复用前，请查阅上游仓库和维护者，了解适用条款。

## 浏览代码

这些链接指向某个特定的源代码快照，因此即使上游开发持续进行，这些说明也仍然有用。它们是学习参考，而非安全评估。

### 为移动中的宠物添加加速和刹车

`petMotionStep` 为每个实体保留速度，利用剩余制动距离限制目标速度，并返回一个单帧大小的移动步长。调用方仍需提供路线并更新变换。

**项目背景：** 在中读取到达距离、加速度和制动值 `shared/config` 在研究其移动手感时，可将其与此辅助函数一起查看。

**源码参考：**

* [`petMotionStep`](https://github.com/dcl-regenesislabs/marsh-colony/blob/55dea4d3e13b3d821aa20bd277fbc4e0bc920b6f/src/client/petMotion.ts#L9-L35)

### 用不同相位为编辑器中放置的道具添加动画

`setupEggShake` 查找命名的蛋实体，记住它们的原始旋转，并为每个蛋应用带有不同相位的小幅正弦波旋转。这在保留每个道具原始朝向的同时增加了变化。

**项目背景：** 它依赖于 Creator Hub 场景生成的蛋名称。

**源码参考：**

* [`setupEggShake`](https://github.com/dcl-regenesislabs/marsh-colony/blob/55dea4d3e13b3d821aa20bd277fbc4e0bc920b6f/src/client/eggShake.ts#L29-L53)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/jiao-cheng-yu-shi-li/published-games/my-dear-pet.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
