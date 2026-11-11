# 实体

世界中的每个单独内容片段（例如一个场景或一件可穿戴物品）都称为一个 *实体*.

实体是不可变的内容包 [文件](/docs/contributor/contributor-zh/nei-rong/file-system/) ，带有一个唯一的字符串标识符，该标识符根据所包含的数据确定性地派生，可用于从内容服务器发现并下载相关文件。

实体的主文件是 *清单*，一个描述实体一般属性以及每个 [类型](#types)。实体的标识符实际上是该 [文件标识符](/docs/contributor/contributor-zh/nei-rong/file-system/#identifiers) 。

由于它们是不可变的，实体不能以传统意义上更新。相反，它们会被新的实体所替代，这些新实体可通过相同稳定的 [指针](/docs/contributor/contributor-zh/nei-rong/pointers/)来发现。实体的最新版本被称为 *已激活*.

每个实体都由所有者签名（该所有者与一个以太坊账户关联）。所有者之后可以使用相同的签名密钥上传实体的新版本，并表明它替换旧版本。内容服务器在接受新实体之前会验证这些签名，无论它们是直接来自客户端还是由另一台服务器转发。

你可以在以下位置查看实际部署的实体： [练习](https://github.com/decentraland/docs/tree/main/contributor/content/practice/README.md) 章节中相同的规则。

### 实体类型 <a href="#types" id="types"></a>

实体共有七种类型：

* [**场景**](/docs/contributor/contributor-zh/nei-rong/shi-ti-lei-xing/scenes/)：世界中的虚拟空间，拥有自己的对象和行为。
* [**个人资料**](/docs/contributor/contributor-zh/nei-rong/shi-ti-lei-xing/profiles/)：关于某个特定玩家的信息，例如他们的名字和头像。
* [**可穿戴物品**](/docs/contributor/contributor-zh/nei-rong/shi-ti-lei-xing/wearables/)：玩家可以添加到其头像上的服装和物品。
* [**表情动作**](/docs/contributor/contributor-zh/nei-rong/shi-ti-lei-xing/emotes/)：玩家头像可以执行的动画。
* [**商店**](/docs/contributor/contributor-zh/nei-rong/shi-ti-lei-xing/stores/)：玩家可以购买的可穿戴物品和表情动作的市场网站。
* [**服装搭配**](/docs/contributor/contributor-zh/nei-rong/shi-ti-lei-xing/outfits/)：某个特定玩家保存的服装搭配。

所有类型在创建、标识、所有权和托管方面都遵循相同的流程。

### 通用属性 <a href="#properties" id="properties"></a>

每个实体在其清单中都有某些适用于所有类型的通用属性。这些顶层字段始终会存在：

| 字段         | 值                                                                              |
| ---------- | ------------------------------------------------------------------------------ |
| `类型`       | 以下之一 `场景`, `资料`, `可穿戴物品`, `表情动作`, `商店` 或 `装扮`.                                 |
| `指针`       | 一个 [指针](/docs/contributor/contributor-zh/nei-rong/pointers/) 与此实体关联。              |
| `时间戳`      | 该实体上传时的 Unix UTC 时间戳。                                                          |
| `内容`       | 指向额外 [文件](/docs/contributor/contributor-zh/nei-rong/file-system/) 文件的引用数组，位于实体包中。 |
| `metadata` | 包含此实体类型特定信息的对象。                                                                |

该字段的结构和值 `metadata` 在每种类型对应的页面中都有详细说明。该 `指针` 数组也会因类型而包含不同的内容。


> [!NOTE]
> 旧的实体清单可能包含 `version` 字段，在 [ADR-45](https://adr.decentraland.org/adr/ADR-45)中已弃用。你可以安全地忽略它，因为 `时间戳` 字段现在用于版本控制。


这是一个描述实体的典型 JSON 清单：

```json
{
  "type": "wearable",
  "pointers": ["urn:decentraland:matic:collections-v2:0xbdf21eaf54ebf4a6cadc2dcb371df7afce98bc1d:0"],
  "timestamp": 1628181913506,
  "content": [
    // ...文件引用，见下文
  ],
  "metadata": {
    // ...此实体类型的特定字段，见相关页面
  }
}
```

你可以在 [通用模式](https://github.com/decentraland/common-schemas) 仓库中找到这些 JSON 结构以及 Decentraland 协议中的其他对象的模式。


> [!NOTE]
> 在查看实体清单时，你可能会发现未文档化的字段。这是因为实体模式允许额外的自定义属性，可由所有者自由设置。


### 文件 <a href="#files" id="files"></a>

如上所述，所有实体至少有一个关联文件：描述实体本身的 JSON 清单。实体标识符实际上是这个特殊文件的 [文件标识符](/docs/contributor/contributor-zh/nei-rong/file-system/#identifiers) 。

该 `内容` 字段是一个指向额外文件的引用数组。这些通常是资源，例如 3D 模型和动画，或场景脚本。

所有文件都存储在 Decentraland 的 [分布式文件系统](/docs/contributor/contributor-zh/nei-rong/file-system/)中，并且数组中的每一项都有两个属性：

| 字段   | 值                                                                                          |
| ---- | ------------------------------------------------------------------------------------------ |
| `文件` | 文件用于相互引用时使用的内部名称。                                                                          |
| `哈希` | 该文件的全局 [标识符](/docs/contributor/contributor-zh/nei-rong/file-system/#identifiers)，在所有内容中都是唯一的。 |

它在 `内容` 字段中通常是这样的：

```json
[
  {
    "file": "thumbnail.png",
    "hash": "bafkreiglecvpnqibvf6pltcnid5nhbx3caj77lu4ia4xitpmp3lrcouuhm",
  },
  {
    "file": "model.glb",
    "hash": "bafkreic2i3awiu7srhatf3k47l3c5lmadisznjigppor2a35saosjfbo25",
  },
  // ...更多文件
]
```


> [!NOTE]
> 该 `文件` 字段值始终使用小写，以避免在不同操作系统中构建实体时出现问题，因为文件名的大小写在这些系统中可能很重要。


文件的生命周期与包含它的实体绑定。对于处于激活状态的实体（即尚未被其所有者替换的实体），协议要求内容服务器保留所有相关文件。如果实体被删除，这些文件可以由服务器自行决定保留或丢弃。

### 所有权与身份验证 <a href="#ownership" id="ownership"></a>

为了证明所有权并授权围绕实体的操作，使用了 [身份验证链](/docs/contributor/contributor-zh/shen-fen-yan-zheng/authchain/) 机制。

该 [`decentraland-crypto`](https://github.com/decentraland/decentraland-crypto) 仓库包含所有加密流程的实现。

### 发现和下载实体

内容服务器可用于通过 [指针](/docs/contributor/contributor-zh/nei-rong/pointers/)定位实体，并下载它们的清单和任何额外文件。

* 要将一个指针解析为实体 ID，你可以使用 [`/entities/active`](https://decentraland.github.io/catalyst-api-specs/#tag/Content-Server/operation/getListOfEntities) 端点。
* 使用实体 ID，你可以通过 [`/contents/<id>`](https://decentraland.github.io/catalyst-api-specs/#tag/Content-Server/operation/getContentFile) 端点。
* 要获取某一类型的所有激活实体，首先下载一个 [快照](/docs/contributor/contributor-zh/nei-rong/snapshots/).

请查看 [练习](https://github.com/decentraland/docs/tree/main/contributor/content/practice/README.md) 部分以获取示例和指南。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-zh/nei-rong/entities.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
