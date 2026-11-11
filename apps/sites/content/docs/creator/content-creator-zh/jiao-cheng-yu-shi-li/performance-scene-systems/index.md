> 资源加载和场景级系统。

# 性能与场景系统

资源加载和场景级系统。

| 示例                                                                                                                    | 它展示了什么               | 维护者          | 标签                | 来源                                                                                               |
| --------------------------------------------------------------------------------------------------------------------- | -------------------- | ------------ | ----------------- | ------------------------------------------------------------------------------------------------ |
| [资源加载](/docs/creator/content-creator-zh/jiao-cheng-yu-shi-li/performance-scene-systems/asset-loading/)                   | 按需加载场景资源，而不是一次性全部加载。 | Decentraland | `性能`, `加载`, `资源`  | [打开场景](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/88,-12-asset-load)      |
| [材质和纹理图库](/docs/creator/content-creator-zh/jiao-cheng-yu-shi-li/performance-scene-systems/material-and-texture-gallery/) | 材质、纹理和 GLTF 节点修改器模式。 | Decentraland | `材质`, `纹理`, `渲染`  | [打开场景](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/52,-52-testing-gallery) |
| [修改器区域](/docs/creator/content-creator-zh/jiao-cheng-yu-shi-li/performance-scene-systems/modifier-areas/)                 | 应用特定区域设置，例如隐藏玩家名称标签。 | Decentraland | `玩家`, `区域`, `修改器` | [打开场景](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/9,99-modifier-areas)    |

[浏览所有示例场景](/docs/creator/content-creator-zh/jiao-cheng-yu-shi-li/examples/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/jiao-cheng-yu-shi-li/performance-scene-systems.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
