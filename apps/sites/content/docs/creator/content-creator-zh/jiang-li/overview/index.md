> Decentraland Rewards 服务通过 HTTP 请求为玩家铸造并发放可穿戴物和表情动作，同时处理交易和费用。

# 概览

该 [奖励 dApp](https://decentraland.org/rewards/) 它是一个强大的工具，可让创作者通过 HTTP 请求使用 Decentraland 穿戴品或表情动作奖励用户。它可用于激励用户、推广场景或提高留存率。奖励可以直接从场景或从服务器发放。

奖励系统会管理在区块链上铸造物品所涉及的大部分复杂性，包括：

* 将多个铸造物品批量打包到一笔交易中，以减少等待时间和费用
* 处理交易费用的支付
* 在区块链失败或重排序时管理重试
* 限制每个用户地址可铸造的物品数量
* 通过验证码系统尽量减少自动化攻击利用
* 通过 Catalyst 节点验证用户是否在场景中
* 监控交易状态
* 在交易确认前告知用户将收到的物品
* 在奖励发放和接收时发送通知
* ![](/docs/_remote/0aefd219f02433f2b3736381.webp)

## 限制

尽管该服务提供了一些自动化防护，以防止一个人批量铸造穿戴品，但仍无法做到 100% 防止被利用。 **不建议使用 Rewards 来铸造稀有度高于 \[EPIC]\(**[**查看文档**](/docs/creator/content-creator-zh/readme/)**#rarity)（100 个或更少副本）**.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/jiang-li/overview.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
