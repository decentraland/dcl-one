# 商店

商店是链下的 [实体](/docs/contributor/contributor-zh/nei-rong/entities/) 它们代表由用户管理的市场站点。它们包含一个描述和一张图片用于向买家展示，以及商店所有者的信息。

### 指针 <a href="#pointers" id="pointers"></a>

商店指针是以下中的 URN： `off-chain` 命名空间中，格式如下：

```
urn:decentraland:off-chain:marketplace-stores:<owner-address>
```

地址段是商店所有者的以太坊地址。例如：

```
urn:decentraland:off-chain:marketplace-stores:0xa2a1dc503be6fdb7878f58f053ded40564e3b9b2
```

### 元数据字段

| 字段            | 值                                                                        |
| ------------- | ------------------------------------------------------------------------ |
| `ID`          | 该 [指针](/docs/contributor/contributor-zh/nei-rong/pointers/) 它解析到（或曾解析到）此商店。 |
| `description` | 此商店的显示标题。                                                                |
| `owner`       | 所有者的以太坊地址。                                                               |
| `图片`          | 一个 `{ file, name }` 引用图片的对象（见下文）。                                        |
| `链接`          | 一个 `{ name, url }` 包含社交媒体或网站链接的对象（见下文）。                                  |

#### 图片

商店会有一张图片在市场中向买家展示，每张图片都有一个用于标识其用途的名称。典型的 `图片` 数组只有一个元素，其名称为 `封面`，如下所示：

```json
[
  {
    "name": "cover",
    "file": "imgs/store-logo.png"
  }
]
```

#### 链接

所有者可以为商店添加外部链接，例如社交媒体资料和外部网站。通常会包含 Facebook 和 Discord 的 URL。示例如下：

```json
[
  {
    "name": "Facebook",
    "url": "https://www.facebook.com/mydclstore"
  },
  {
    "name": "网站",
    "url": "https://mydclstore.com"
  }
]
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-zh/nei-rong/shi-ti-lei-xing/stores.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
