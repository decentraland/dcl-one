# 指针

指针是唯一的、不区分大小写的字符串，用于引用一个活动的 [实体](/docs/contributor/contributor-zh/nei-rong/entities/)。内容服务器可以解析这些指针以获取实体的标识符。

请记住，两者 [实体](/docs/contributor/contributor-zh/nei-rong/entities/) 及其 [文件](/docs/contributor/contributor-zh/nei-rong/file-system/) 在 Decentraland 内容系统中都是不可变的，并且当上传替代版本时，它们的标识符会改变。另一方面，指针是稳定的引用，会在替换版本之间保持不变。这是通过在所有者上传替代版本时自动将指针重定向到新实体来实现的。

多个指针可以解析到同一个 [实体](/docs/contributor/contributor-zh/nei-rong/entities/)，这在 [场景](/docs/contributor/contributor-zh/nei-rong/shi-ti-lei-xing/scenes/).

如你所见，指针最常见的用途是获取一个活动版本（即最新替代版本）的 ID [实体](/docs/contributor/contributor-zh/nei-rong/entities/)，以便下载它。

### 指针类型

内容服务器可以将 4 种类型的指针解析为实体，每种都有其自己的语法：

* [场景指针](/docs/contributor/contributor-zh/nei-rong/shi-ti-lei-xing/scenes/#pointers) 是地块坐标，例如 `"0,0"`.
* [资料指针](/docs/contributor/contributor-zh/nei-rong/shi-ti-lei-xing/profiles/#pointers) 是所有者的以太坊地址。
* [可穿戴物指针](/docs/contributor/contributor-zh/nei-rong/shi-ti-lei-xing/wearables/#pointers) 和 [表情指针](/docs/contributor/contributor-zh/nei-rong/shi-ti-lei-xing/emotes/#pointers) 要么是 [collection](/docs/contributor/contributor-zh/nei-rong/collections/) 物品 URN 或链下 URN。
* [商店指针](/docs/contributor/contributor-zh/nei-rong/shi-ti-lei-xing/stores/#pointers) 是带有所有者以太坊地址的链下 URN。
* [装扮指针](/docs/contributor/contributor-zh/nei-rong/shi-ti-lei-xing/outfits/#pointers) 是所有者的以太坊地址，后缀为 `:outfits`.

你可以在各自的专门章节中找到更多详情。

### 解析指针

内容服务器有一个端点，可以通过发起以下 `GET` 请求：

```
https://<content-base-url>/entities/<entity-type>?pointer=<pointer>
```


> [!NOTE]
> 由于历史原因，该端点的响应是一个包含一个元素的数组。


以下是一些有用的示例，使用 Foundation 的内容服务器：

```bash
# 一个指向可穿戴物的链上集合指针：
curl "https://peer.decentraland.org/content/entities/wearables/?pointer=urn:decentraland:matic:collections-v2:0x30517529cb5c16f686c6d0b48faae5d250d43005:0"

# 一个指向默认资产的链下集合指针：
curl https://peer.decentraland.org/content/entities/wearables/?pointer=urn:decentraland:off-chain:base-avatars:BaseFemale

# 一个地块指针：
curl https://peer.decentraland.org/content/entities/wearables/?pointer=0,0

# 一个资料指针：
curl https://peer.decentraland.org/content/entities/profiles/?pointer=0xe2c2b80ca5ad868f4b30fa83cca2bf12cc95b4fe
```

请注意，如果你已经知道实体的 ID，你可以 [直接下载清单](/docs/contributor/contributor-zh/nei-rong/file-system/#downloading) 文件。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-zh/nei-rong/pointers.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
