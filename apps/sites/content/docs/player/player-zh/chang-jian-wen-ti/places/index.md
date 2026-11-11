> 地点

# 地点

<details>

<summary>什么是 Places？</summary>

[地点](https://decentraland.org/places/) 会显示 Genesis City 和 Worlds 中的某些兴趣点。

</details>

<details>

<summary>如何编辑我的 Place 的标题、描述或图片？</summary>

你的 Place 的标题、描述和图片均取自场景元数据。请参阅 Creator 文档，了解更多关于编辑场景元数据的信息。

</details>

<details>

<summary>我的 Place 会如何更新？</summary>

当你重新上传场景时，Places 网站会自动更新任何元数据。

</details>

<details>

<summary>如果我对我的 Place 做出重大更改，会发生什么？</summary>

每个 Place 都会被分配一个唯一的 ID。当满足某些条件时，你的 Place 可能会被视为“新”的，从而分配一个新的 ID。此更改会导致你的 Place 失去所有累计的收藏和点赞。有关这些条件的详细信息，请参阅 [**ADR：Place 标识符**](https://adr.decentraland.org/adr/ADR-186).

</details>

<details>

<summary>点赞率是如何计算的？</summary>

为防止滥用，点赞率是根据投票权至少为 100 的用户的点赞和点踩计算得出的。任何拥有账户的人都可以点赞或点踩，但只有符合条件的投票才会用于计算点赞率。

确保你的 Place 具有标题、描述和缩略图，以提高获得好评的几率。

</details>

<details>

<summary>我的 Place 如何成为兴趣点？</summary>

兴趣点是 Decentraland 中值得注意的位置。这些“POI”会在虚拟世界界面的多个区域中被推荐，并被认为是用户探索的好地方，尤其适合刚接触 Decentraland 的用户。

你可以在 [**DAO**](https://decentraland.org/dao/en/) 来提名你的 Place 作为兴趣点。

</details>

<details>

<summary>哪些 Worlds 会显示在 Worlds 选项卡中？</summary>

如 *Worlds 1.0 - 短期计划* [**DAO 提案**](https://decentraland.org/governance/proposal/?id=e712bb50-e822-11ed-b8f1-75dbe089d333/)中所定义，我们仅允许那些同时持有 LAND 或有效 LAND 租赁合同的 NAME 所有者发现部署到 Foundation 的 World Content Server 的 Worlds。

</details>


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/player/player-zh/chang-jian-wen-ti/places.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
