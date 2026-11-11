> 从 Rewards 活动中领取奖励的可穿戴物发放器和空投箱。

# 奖励领取发放器

向玩家发放来自 Decentraland 奖励活动的可穿戴物品。场景包括一个点击领取发放器、一个按钮发放器，以及一个可空投可领取箱子的拉杆。

## 项目信息

**维护者：** Decentraland

[查看源代码](https://github.com/decentraland/sdk7-goerli-plaza/tree/main/reward-claim)

**许可证：** [Apache-2.0](https://github.com/decentraland/sdk7-goerli-plaza/blob/main/reward-claim/LICENSE)。在重复使用前请检查上游条款；代码和资源可能适用不同条款。

## 浏览代码

这些链接指向某个特定的源代码快照，因此即使上游开发持续进行，这些说明也仍然有用。它们是学习参考，而非安全评估。

### 使用签名 fetch 请求奖励

`claimToken` 限制重复点击，跳过本次会话中已成功领取的发放器，并检查玩家是否拥有钱包。 `requestToken` 然后使用以下方式将发放器密钥、当前 catalyst 和玩家地址发送到奖励服务器： `signedFetch`.

**项目背景：** 测试默认开启（`CONFIG_CLAIM_TESTING_ENABLED` 位于 `src/config.ts`），它使用内置的测试活动。若要使用你自己的，请替换其中的占位活动 ID 和发放器密钥： `ClaimConfig` 并关闭测试。有关创建活动和发放器，请参阅 Rewards 入门指南。

**源码参考：**

* [`claimToken` 和 `requestToken`](https://github.com/decentraland/sdk7-goerli-plaza/blob/54a07775ee340f0ce22eb23280ef0e9c35f5e386/reward-claim/src/dispenser/claiming/claim.ts#L26-L99)
* [`ClaimConfig`](https://github.com/decentraland/sdk7-goerli-plaza/blob/54a07775ee340f0ce22eb23280ef0e9c35f5e386/reward-claim/src/dispenser/claiming/claimConfig.ts#L21-L34)

### 将 Rewards 的响应映射为玩家反馈

`processResponse` 对不可用活动、库存已空的发放器、已分配的奖励以及成功领取显示不同的发放器界面。未启动、已禁用和已完成的活动共用一个分支；只有当活动配置设置了以下字段时，才会显示未开始或已结束界面： `startDate` 或 `endDate`；否则显示通用的已完成界面。

**项目背景：** 这些 UI 状态定义在 `dispenserUi.tsx`。活动开始和结束日期是活动配置中的可选字段。

**源码参考：**

* [`processResponse`](https://github.com/decentraland/sdk7-goerli-plaza/blob/54a07775ee340f0ce22eb23280ef0e9c35f5e386/reward-claim/src/dispenser/claiming/claim.ts#L101-L197)

### 使用验证码重试领取

当 Rewards 服务器要求验证码时， `getCaptcha` 会获取验证码图片并在发放器界面中显示。提交答案会再次调用 `claimToken` ，并带上验证码 ID 和值。

**项目背景：** 是否需要验证码是 Rewards 控制面板中该发放器的一个设置。

**源码参考：**

* [`getCaptcha`](https://github.com/decentraland/sdk7-goerli-plaza/blob/54a07775ee340f0ce22eb23280ef0e9c35f5e386/reward-claim/src/dispenser/claiming/claim.ts#L209-L216)
* [验证码提交按钮](https://github.com/decentraland/sdk7-goerli-plaza/blob/54a07775ee340f0ce22eb23280ef0e9c35f5e386/reward-claim/src/dispenser/claiming/ui/dispenserUi.tsx#L640-L643)

### 从空投箱子中领取

`randomCrateSpawn` 会在舞台区域中的随机位置投放一个带降落伞的箱子。 `DropCrate` 向下进行射线检测以找到地面，并通过补间将箱子放到地面上；点击箱子即可领取奖励并使其消失。

**项目背景：** 箱子投放由以下对象触发： `Activate` 位于以下位置的 Creator Hub 拉杆的操作： `main`.

**源码参考：**

* [`randomCrateSpawn` 和 `SpawnCrate`](https://github.com/decentraland/sdk7-goerli-plaza/blob/54a07775ee340f0ce22eb23280ef0e9c35f5e386/reward-claim/src/crate.ts#L40-L119)
* [`DropCrate`](https://github.com/decentraland/sdk7-goerli-plaza/blob/54a07775ee340f0ce22eb23280ef0e9c35f5e386/reward-claim/src/crate.ts#L142-L189)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/jiao-cheng-yu-shi-li/rewards/reward-claim-dispenser.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
