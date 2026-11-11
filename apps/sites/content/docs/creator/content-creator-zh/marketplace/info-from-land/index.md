> 使用 CLI 获取地块、庄园或地址的信息。

# 来自 LAND 的信息

你可以使用 Decentraland CLI 直接从区块链中的 LAND 代币以及上传到内容服务器的场景文件中查询信息。

要运行这些命令，你必须先 [安装 CLI](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/using-the-cli/).

## 获取场景信息

该 `dcl info` 命令返回某个场景的 *scene.json* 文件内容，包括所有者、联系信息以及场景中的地块。

* `dcl info` 从场景文件夹中返回该特定场景的信息。
* `dcl info x,y`，其中 *x* 和 *y* 是地块坐标，返回该位置场景的信息。

  {% hint style="warning" %} **📔 注意**: 不要在两个坐标之间添加空格。 {% endhint %}
* `dcl info id`，其中 *id* 是一个庄园 ID，返回与该庄园 ID 对应场景的信息。

该 `dcl status` 命令返回一个列表，列出已部署到场景的文件。这里只包含文件名和大小。你无法通过 CLI 访问这些文件的内容。

* `dcl status` 从场景文件夹中返回该特定场景的信息。
* `dcl status x,y`，其中 *x* 和 *y* 是地块坐标（中间不含空格），返回该位置场景的信息。


> [!WARNING]
> **📔 注意**: 不要在两个坐标之间添加空格。


* `dcl status id`，其中 *id* 是一个庄园 ID，返回与该庄园 ID 对应场景的信息。


> [!WARNING]
> **📔 注意**：上传到我们内容服务器的所有内容都是公开的，并且可通过该网络访问。部署场景时，默认情况下你上传的不是该场景的原始源代码，而是一个编译并压缩后的 JavaScript 版本，可读性要差得多。


## 从钱包地址获取信息

该 `dcl info` 命令还可以让你查看某个以太坊钱包及其拥有的 LAND 代币的信息。

* `dcl info xxxx`，其中 *xxxx* 是你感兴趣的以太坊地址，返回该地址拥有的所有 LAND 地块和庄园及其详细信息。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/marketplace/info-from-land.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
