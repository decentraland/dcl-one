> 一种可穿戴物发放器，玩家可在触发区域内做拍手表情来领取。

# 拍手领取发放器

让玩家在发放器前做拍手表情即可领取一个可穿戴物品，这是 Metaverse Fashion Week 使用的一种机制。README 会指导如何将发放器指向你自己的 Rewards 活动。

## 项目信息

**维护者：** Decentraland

[查看源代码](https://github.com/decentraland/sdk7-goerli-plaza/tree/main/clap-claim-dispenser)

**许可证：** 本目录中未标识。在重新使用前，请先查阅上游仓库和维护者以了解适用条款。

## 浏览代码

这些链接指向特定的源码快照，因此即使上游持续开发，相关说明仍然有用。它们是学习参考，而非安全评估。

### 当玩家站在发放器前时，为发放器装载武器

每个发放器都会将其活动存储在一个 `DispenserComponent`中。前方的一个球形 `TriggerArea` 会将其设置为 `isActive` ，并在进入时显示“拍手领取”提示，在离开时清除这两者。

**项目背景：** 活动 ID 和发放器密钥来自 `ClaimConfig` 位于 `src/modules/claiming/claimConfig.ts`.

**源码参考：**

* [`createDispenserWithWearable`](https://github.com/decentraland/sdk7-goerli-plaza/blob/54a07775ee340f0ce22eb23280ef0e9c35f5e386/clap-claim-dispenser/src/modules/dispensers.ts#L28-L89)

### 当玩家播放拍手表情时领取

`initClapToClaim` 监听 `AvatarEmoteCommand` 在玩家实体上。当表情是 `clap`时，它会等待动画完成，并从当前处于激活状态的每个发放器领取。

**项目背景：** 领取请求和响应处理遵循与奖励领取发放器示例相同的 Rewards 流程。

**源码参考：**

* [`initClapToClaim`](https://github.com/decentraland/sdk7-goerli-plaza/blob/54a07775ee340f0ce22eb23280ef0e9c35f5e386/clap-claim-dispenser/src/modules/clapToClaim.ts#L6-L33)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/jiao-cheng-yu-shi-li/rewards/clap-to-claim-dispenser.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
