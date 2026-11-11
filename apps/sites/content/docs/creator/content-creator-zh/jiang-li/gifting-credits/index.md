> 赠送点数是一个仅限受邀参与的测试项目：参与的工作室会通过商店的工作室页面向玩家赠送点数。

# 赠送点数（测试版）

赠送 Credits 目前是仅限受邀参与的测试计划。参与的工作室可以向其玩家赠送 Credits。

如果你的工作室已受邀参与，你可以通过商店的工作室页面在 [decentraland.org/shop/studio](https://decentraland.org/shop/studio)发送 Credits。玩家可以在商店中用这些 Credits 购买任何物品。

## 开始前

* **你的账户必须管理该工作室。** Decentraland 会添加可以为你的工作室赠送的账户。如果你登录后看到 *此账户不管理工作室*，请联系你的 Decentraland 联系人添加该账户。
* **你工作室自己的账户不能接收其 Credits。** 请使用其他账户进行任何测试赠送。
* **赠送是最终的。** 已赠送的 Credits 永不过期，也无法撤回，所以在确认前请先检查列表。

## 你的工作室页面

在以下地址登录工作室页面： [decentraland.org/shop/studio](https://decentraland.org/shop/studio)。如果你的账户管理多个工作室，请从 **工作室** 菜单中选择。页面会显示：

* **剩余预算**：你的工作室还能赠送的 Credits。
* **已赠送**：你的工作室已赠送的金额，以及赠送次数。
* **每位玩家最多**：单个玩家一次最多可获得的赠送金额。
* **赠送记录**：你的工作室进行的每次赠送，包含玩家、Credits、原因、日期以及发送者。

![工作室页面，显示剩余预算、已赠送的 Credits、每位玩家最多可获赠金额以及赠送列表](/docs/_remote/eca2a726b6b3c94c7710e0b5.webp)

如果页面显示工作室已暂停，则在 Decentraland 恢复之前无法赠送。

## 赠送 Credits

1. 按 **赠送 Credits**.
2. 填写一个 **原因**。它会用于所有没有自己原因的玩家：一行，最多 80 个字符，例如 *本周最佳玩家*.
3. 添加玩家，每行一个：他们的账户（ `0x…` 地址）以及要赠送的 Credits。你也可以为某一行单独设置原因。
   * **添加玩家** 会添加一行。
   * **粘贴列表** 每行一个玩家：账户、Credits 以及可选原因。从电子表格复制的列也可以。

     ```
     0x…，100，本周最佳玩家
     0x…，50
     ```
   * 填写金额时不要使用千位分隔符（`1000`，而不是 `1,000`).
   * 每个账户只能占一行：如果某位玩家获得两个金额，请将它们相加。
4. 检查列表下方的摘要（玩家、Credits 总额以及之后剩余预算），然后按 **审核**.

   ![带有原因和两位玩家的赠送 Credits 窗口，每位玩家都有自己的 Credits 和原因](/docs/_remote/24ad20ec86951563169233a0.webp)
5. 审核会提醒你：过去 30 天内，你的工作室是否向任何玩家赠送过相同的礼物（相同的 Credits 和原因）。除非你确实要再次赠送，否则请返回并移除他们。
6. 输入总 Credits 以确认，然后按 **赠送给 … 位玩家**.

   ![确认步骤，已输入总 Credits，并显示“赠送给 2 位玩家”按钮](/docs/_remote/f677deb1676df2cd4f1c5912.webp)

礼物会逐个发送，每一行都会显示处理结果。如果窗口关闭或你停止了列表，请再次打开 **赠送 Credits** 并按 **继续**：已经收到礼物的玩家不会重复收到。

## 你的玩家会收到什么

* Decentraland 中的一条通知，告知你的工作室已向他们赠送 Credits。
* 一封电子邮件，如果他们已在自己的 Decentraland 账户中确认了邮箱。
* 商店活动中的该礼物，以及余额中的 Credits，可立即使用。

## 如果赠送未成功

| 消息         | 该怎么做                                                    |
| ---------- | ------------------------------------------------------- |
| 此账户属于你的工作室 | 你工作室自己的账户不能接收其 Credits。移除该行。                            |
| 此账户无法接收赠送  | 此玩家无法接收 Credits。移除该行。                                   |
| 超过每位玩家上限   | 将金额调低到 **每位玩家最多** 你工作室页面上显示的数值。                         |
| 剩余预算不足     | 少赠送一些，或者与你的 Decentraland 联系人讨论你的预算。                     |
| 工作室已暂停     | 等待 Decentraland 恢复该工作室，然后按 **继续**.                      |
| 我们未收到确认    | 查看 **赠送记录** 在再次发送前先检查列表：礼物可能已经送达。按 **继续** 绝不会向同一个人赠送两次。 |


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/jiang-li/gifting-credits.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
