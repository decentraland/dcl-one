# CLI 示例

本练习演示如何在终端中使用 content API。我们将使用 Decentraland Foundation 的实例： `peer.decentraland.org`，你只需要 `curl` 命令行工具。


> [!NOTE]
> 你可以使用 [`httpie`](https://github.com/httpie/httpie) 或者用 [`jq`](https://github.com/stedolan/jq) 来让每次请求返回更易读的 JSON。在本指南中，为了便于理解，我们将展示格式化后的文档。


我们要做的是：

1. 查询内容服务器的状态。
2. 查找并下载每日 [快照](/docs/contributor/contributor-zh/nei-rong/snapshots/) ，其中包含一个实体列表。
3. 获取某个实体的清单。
4. 下载该实体的某个文件。

首先，让我们使用以下命令查询服务器状态： `/about`:

```bash
curl "https://peer.decentraland.org/about"
```

```js
{
  "healthy": true,
  "content": {
    "healthy": true,
    "version": "6.5.0",
  },
  // ... 更多服务器信息（功能标志、版本、路径等）
}
```

看起来服务器已正常运行（`"healthy": true`），并且向我们提供了它为每个功能集实现的版本信息，以及该实例的一些配置选项。

我们想下载一些内容，所以先来看看可用的 [快照](/docs/contributor/contributor-zh/nei-rong/snapshots/) 以便通过 `/content/snapshots`.

```bash
curl "https://peer.decentraland.org/content/snapshots"
```

```js
[
  {
    "hash": "bafybeia6qoum64psaooiqo3f45i6hykfwx723uc236waub3gng2naof224",
    "timeRange": {
      "initTimestamp": 1689120000000,
      "endTimestamp": 1689206400000
    },
    "replacedSnapshotHashes": [],
    "numberOfEntities": 981,
    "generationTimestamp": 1689219353866
  },
  // ...更多快照文件
]
```

数组中的每一项都描述了一个 [快照](/docs/contributor/contributor-zh/nei-rong/snapshots/)。让我们获取一个 `哈希` ，并从 `/content/contents` 端点。

```bash
curl "https://peer.decentraland.org/content/contents/bafybeia6qoum64psaooiqo3f45i6hykfwx723uc236waub3gng2naof224" > snapshot
```


> [!NOTE]
> 你可以尝试更大的快照，例如 [高级 Python 示例](https://github.com/decentraland/documentation/blob/main/content/contributor/content/practice/snapshots.py)。若想以交互方式尝试，最好选择较小的快照。


我们可以通过查看第一行，确认自己下载的是正确的文件，并且格式是我们熟悉的：

```bash
head -n1 snapshot
```

```
### Decentraland JSON 快照
```

太好了！现在我们已经在本地获得了该快照中捕获的所有实体摘要。让我们取列表中的第一个（文件中的第二行），一个 `资料`:

```bash
tail -n+2 snapshot | head -n1 
```

```js
{
  "entityId": "bafkreif7hjremkxlvixyxoxnoo7bdcnf7qqp245sjb2pag2nk3n6o6yc4c",
  "entityType": "profile",
  "pointers": [
    "0x271cdb3b1c792c336c4b2bdc52c4f415d0046b92"
  ],
  "authChain": [
    // See https://docs.decentraland.org/contributor/auth/authchain/
  ],
  "entityTimestamp": 1689120135624
}
```


> [!NOTE]
> 由于快照会过期，实体也会被替换，本文中的标识符将不起作用。请在命令行中跟着操作，以获得真实、有效的文件 ID。


这些信息我们可以保存下来。我们将使用 `entityId` 来下载该实体的 JSON 清单，但持久保存列出的 [指针](/docs/contributor/contributor-zh/nei-rong/pointers/) 内容是个好主意，这样将来我们就能定位到这个实体及其任何更新版本。

我们还有 [身份验证链](/docs/contributor/contributor-zh/nei-rong/entities/#ownership) 用于对该实体签名的内容，而且我们可以验证列出的签名，以确认我们下载的任何相关文件的真实性。

让我们获取该实体的清单。记住， `entityId` 是 [文件标识符](/docs/contributor/contributor-zh/nei-rong/file-system/#identifiers) 我们需要的内容，我们可以使用 `/content/contents` 端点再次获取：

```bash
curl "https://peer.decentraland.org/content/contents/bafkreigcreq7rv6b2wf4zc4fsnif43ziwb4q46v4qhsewpf7gbsyxew3om"
```

```js
{
  "version": "v3",
  "type": "profile",
  "pointers": [
    "0x273cdb3b1c791c336c4b2bcc52c4f415d0046b91"
  ],
  "timestamp": 1689120135624,
  "content": [
    {
      "file": "body.png",
      "hash": "bafybeibzaqkirz7fk474xvyhurho5xviphs7anawaceb6gscuigia4x33u"
    },
    // ...更多文件
  ],
  "metadata": {
    // ...头像和其他信息
  }
}
```

这个 `资料` 实体包含 World Explorers 用于渲染和让玩家动起来的所有信息。如果我们想获取其中一个打包文件，仍然可以继续使用 `/content/contents` 端点。

如果我们查看 `内容` 字段，就能看到 `哈希` 这个文件在内部被称为 `body.png`。让我们把它取出来：

```bash
curl "https://peer.decentraland.org/content/contents/bafybeibzaqkirz7fk474xvyhurho5xviphs7anawaceb6gscuigia4x33u" > body.png
```

我们可以打开这个 `png` 文件，在图片查看器或网页浏览器中查看作者的作品。不错！


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-zh/nei-rong/shi-jian/cli-examples.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
