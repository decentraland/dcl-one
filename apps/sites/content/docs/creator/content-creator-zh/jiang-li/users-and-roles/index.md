> 通过为钱包地址分配角色和权限，管理谁可以操作 Decentraland Rewards 活动。

# 用户与角色

要在 Rewards 服务中的某个活动上执行任何操作，您必须使用具有必要权限的钱包登录。

当您创建活动时，系统会自动将您指定为所有者，并授予您所有权限。作为所有者，您可以将其他用户添加到该活动中，并为他们分配具有特定权限的角色。不过，每个活动只能有一位所有者——创建者——而且只有该用户才有权限管理资金。

向该活动添加用户。

1. 点击 **用户角色** 选项卡以查看有权访问该活动的用户列表。
2. 在此列表末尾，您会找到一个用于添加新用户的表单。注意：只有在您拥有添加用户所需的权限时，才会显示该表单。
3. 填写表单，输入用户的钱包地址（或 ENS 名称）并选择其角色。
4. 点击 **添加用户** 用于完成添加的按钮。

![](/docs/_remote/f926b41052805cdf281f1c90.webp)

## 角色权限

以下是各角色允许执行的操作：

| 权限               | 查看者 | 开发者 | 协作者 | 所有者 |
| ---------------- | :-: | :-: | :-: | :-: |
| 查看活动名称、网络和最大 gas |  ✅  |  ✅  |  ✅  |  ✅  |
| 编辑活动名称和最大 gas    |  ❌  |  ❌  |  ✅  |  ✅  |
| 启用和停用活动          |  ❌  |  ❌  |  ✅  |  ✅  |
| 查看供应量            |  ✅  |  ✅  |  ✅  |  ✅  |
| 添加/移除供应量         |  ❌  |  ✅  |  ✅  |  ✅  |
| 查看发放器            |  ✅  |  ✅  |  ✅  |  ✅  |
| 查看发放器密钥          |  ❌  |  ✅  |  ✅  |  ✅  |
| 添加/移除发放器         |  ❌  |  ✅  |  ✅  |  ✅  |
| 查看活动交易           |  ✅  |  ✅  |  ✅  |  ✅  |
| 查看用户             |  ✅  |  ✅  |  ✅  |  ✅  |
| 添加/移除用户          |  ❌  |  ❌  |  ✅  |  ✅  |
| 移除所有者用户          |  ❌  |  ❌  |  ❌  |  ✅  |
| 管理资金             |  ❌  |  ❌  |  ❌  |  ✅  |


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/jiang-li/users-and-roles.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
