> 核心玩法机制和玩家交互。

# 玩法与交互

核心玩法机制和玩家交互。

| 示例                                                                                                                   | 演示内容                         | 维护者          | 标签                 | 源代码                                                                                                  |
| -------------------------------------------------------------------------------------------------------------------- | ---------------------------- | ------------ | ------------------ | ---------------------------------------------------------------------------------------------------- |
| [输入修饰器](/docs/creator/content-creator-zh/jiao-cheng-yu-shi-li/gameplay-interaction/input-modifier/)                     | 在场景的某一部分改变或限制玩家移动输入。         | Decentraland | `移动`, `输入`, `玩家`   | [打开场景](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/0,1-input-modifier)         |
| [粒子系统](/docs/creator/content-creator-zh/jiao-cheng-yu-shi-li/gameplay-interaction/particle-system/)                     | 火焰、烟雾、火花、雪和其他粒子特效。           | Decentraland | `粒子`, `特效`, `视觉效果` | [打开场景](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/0,7-particle-system)        |
| [指针锁定控制](/docs/creator/content-creator-zh/jiao-cheng-yu-shi-li/gameplay-interaction/pointer-lock-control/)              | 捕获鼠标输入以实现自定义控制。              | Decentraland | `输入`, `鼠标`, `摄像机`  | [打开场景](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/31,20-pointer-lock-control) |
| [射线检测与碰撞体层](/docs/creator/content-creator-zh/jiao-cheng-yu-shi-li/gameplay-interaction/raycasting-and-collider-layers/) | 指向场景几何体，以及控制哪些碰撞体会对射线检测作出响应。 | Decentraland | `交互`, `射线检测`, `碰撞` | [打开场景](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/77,-1-raycast-unit-tests)   |
| [触发区域](/docs/creator/content-creator-zh/jiao-cheng-yu-shi-li/gameplay-interaction/trigger-areas/)                       | 检测玩家进入和离开区域，包括基于碰撞体的触发器。     | Decentraland | `交互`, `触发器`, `碰撞`  | [打开场景](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/75,-9-trigger-areas)        |
| [Zombie Attack](/docs/creator/content-creator-zh/jiao-cheng-yu-shi-li/gameplay-interaction/zombie-attack/)              | 一个完整、可玩的游戏场景，包含敌人和计分系统。      | Decentraland | `游戏`, `完整场景`, `分数` | [打开场景](https://github.com/decentraland/sdk7-goerli-plaza/tree/main/zombie-attack)                    |

[浏览所有示例场景](/docs/creator/content-creator-zh/jiao-cheng-yu-shi-li/examples/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/jiao-cheng-yu-shi-li/gameplay-interaction.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
