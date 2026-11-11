# 文件系统

在 Decentraland 协议中，不同内容 API 的底层是一个分布式文件系统的实现。每个内容服务器都保存着整个存储的一份副本，并将其接收到的更新与其他实例同步。

上传到系统中的文件是 **不可变的**。一旦部署，它们的标识符和内容都不会改变。它们只能以这样的方式更新：上传一个新版本，由应用选择使用它，而旧版本可以被丢弃。

你可以在 [练习](https://github.com/decentraland/docs/tree/main/contributor/content/practice/README.md) 部分。

### 文件标识符 <a href="#identifiers" id="identifiers"></a>

Decentraland 的文件系统组织为一个扁平索引（即没有自然的目录层级），其中每个文件都由一个唯一字符串标识。

标识符字符串的前缀是 [base32](https://en.wikipedia.org/wiki/Base32) 文件内容 SHA-256 哈希的编码，使用 [IPFS CID v1](https://docs.ipfs.tech/concepts/content-addressing/) 算法。它们看起来像这样：

```
bafybeicgclohdfaccu2sqqkzrzuenjxzcry3m5vcb4mpxgucjl3oheq5tq
```

前几个字节中的前缀表示标识符本身的编码、版本和哈希类型。


> [!NOTE]
> 出于历史原因，一些 API 将文件标识符称为 *哈希* 而不是 *ID*，但它们指的是同一件事。你也可能在旧内容中找到看起来稍有不同的旧式标识符。


在实际使用中，发现和下载内容并不需要这些细节。该协议的大多数客户端可以将标识符视为不透明字符串，而不会失去任何能力。

请注意，现代 CIDv1 标识符与 IPFS 兼容，同一文件会生成相同的 CID，因此可以使用 IPFS 作为文件提供方。

### 所有权与持久性

除了 [快照](/docs/contributor/contributor-zh/nei-rong/snapshots/)，网络中存储的文件都关联到一个 [实体](/docs/contributor/contributor-zh/nei-rong/entities/) ，该实体由一个以太坊账户拥有。只有所有者可以更新该实体及相关文件。

协议要求内容服务器始终存储实体及其文件的最新版本，但可以根据各自的配置选择是否保留旧版本。

### 下载文件 <a href="#downloading" id="downloading"></a>

该 [`/contents/<fileId>`](https://decentraland.github.io/catalyst-api-specs/#tag/Content-Server/operation/getContentFile) 内容服务器的端点可用于下载任何文件。它们的标识符可在父级中找到 [实体的](/docs/contributor/contributor-zh/nei-rong/entities/)清单中。

### 上传文件

文件不会单独上传。相反，它们被打包在实体中，并在将实体部署到内容服务器时上传。

请参阅 [内容创建者文档](https://github.com/decentraland/docs/tree/main/creator/README.md) 了解更多详情。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-zh/nei-rong/file-system.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
