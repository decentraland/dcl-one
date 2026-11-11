# 如何创建新的实体类型

#### 什么时候需要创建一个新实体？

首先，检查现有实体是否可以解决所需的用例，以避免对解决方案进行过度设计。如果 <https://github.com/decentraland/common-schemas/blob/main/src/platform/entity.ts#L13> 中定义的现有实体类型都不满足要求，那么你可能需要创建一个新的实体类型。

#### 首先回答：

* 新的实体是一个 NFT 合集吗？
* 所有权将如何验证？
* 它的大小会是多少？
* 将创建多少个实体？

#### 步骤

首先在 <https://github.com/decentraland/adr> 中创建一个 ADR，在其中定义 schema 以及开始验证新实体的日期。

**仓库**

**@dcl/schemas**

1. 在枚举中创建新的实体类型：<https://github.com/decentraland/common-schemas/blob/main/src/platform/entity.ts#L19>
2. 在 <https://github.com/decentraland/common-schemas/tree/main/src/platform> 中创建文件夹和文件，生成类型和 JSON schema，例如：<https://github.com/decentraland/common-schemas/blob/main/src/platform/profile/profile.ts#L20>

**@dcl/content-validator**

实现 ADR 中定义的验证，同时考虑该 ADR 的日期，在 <https://github.com/decentraland/content-validator/blob/main/src/validations/timestamps.ts> 中进行处理。例如：<https://github.com/decentraland/content-validator/blob/main/src/validations/items/emotes.ts#L8>

1. 在 <https://github.com/decentraland/content-validator/tree/main/src/validations/access-checker> 中添加代码，以检查新实体类型的所有权
2. 为新类型实现 validationForType，例如：<https://github.com/decentraland/content-validator/blob/main/src/validations/profile.ts#L112>
3. 在这里你需要在 <https://github.com/decentraland/content-validator/blob/main/src/validations/ADR51.ts> 中定义每个实体的最大大小。

**@dcl/urn-resolver**

1. 如果新的实体对应一个合集，那么无需添加任何内容。
2. 如果不是，则创建一个与新实体对应的新 urn，并在 <https://github.com/decentraland/urn-resolver> 中定义它，同时在 <https://github.com/decentraland/urn-resolver/blob/main/src/resolvers.ts#L23> 中添加一个 resolver

**Catalyst：Content Server**

1. 数据库无需更改。
2. 更新上面提到的所有已修改库。

**Catalyst：Lambdas Server**

1. 更新上面提到的所有已修改库。
2. 检查是否需要更新 profile 清理（`/lambdas/profiles` 端点）
3. 检查 erc collections 端点是否需要更新（`/lambdas/collections` 端点）
4. 创建一个端点来展示该实体，例如 `/lambdas/wearables-by-owner`


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-zh/jiao-cheng/how-to-create-new-entity-type.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
