# 快照

内容服务器会定期汇总其托管的活动实体，这些汇总称为 *快照*。它们是普通的 [文件](/docs/contributor/contributor-zh/nei-rong/file-system/) ，并且可以使用其标识符下载。

快照按日、周、月和年生成。每个快照都包含自上一份同时间范围的快照以来发生变化的活动实体集合。

快照会包含同一实体的冲突版本（即不同的 [清单文件](/docs/contributor/contributor-zh/nei-rong/entities/#properties) 关联到同一个指针），因为它们会被更新。在扫描时，客户端应保留最新快照中的版本。由于内容服务器允许删除非活动文件，过时的实体版本不保证可供下载。

当一份新快照 *替换* 较旧的快照时（例如，合并了一系列日快照的周快照），其元数据会指示哪些先前文件已被替换，因此客户端无需下载它们。

通过组合所有可用的快照（下文会详细说明），并保留沿途发现的每个 [指针](/docs/contributor/contributor-zh/nei-rong/pointers/) 所引用的最新实体，可以发现全部活动实体集合。

你可以在 [练习](https://github.com/decentraland/docs/tree/main/contributor/content/practice/README.md) 部分使用可运行的代码来试验快照。

### 发现快照 <a href="#discover" id="discover"></a>

要定位当前的快照集合，请使用 [`快照` 端点](https://decentraland.github.io/catalyst-api-specs/#tag/Content-Server/operation/getSnapshots)。响应包含一个项目数组，字段如下：

| 字段                        | 值                                                             |
| ------------------------- | ------------------------------------------------------------- |
| `generationTimestamp`     | 此快照创建时的 Unix UTC 时间戳。                                         |
| `hash`                    | 快照 [文件](/docs/contributor/contributor-zh/nei-rong/file-system/). |
| `numberOfEntities`        | 快照文件中的条目数量。                                                   |
| `replacedSnapshotHashes`  | 一个数组，包含 `hash` 被此快照替换的任何快照的哈希值。                               |
| `timeRange.initTimestamp` | 快照范围开始时的 Unix UTC 时间戳（毫秒）。                                    |
| `timerange.endTimestamp`  | 快照范围结束时的 Unix UTC 时间戳（毫秒）。                                    |

例如：

```json
{
  "generationTimestamp": 1684979298844,
  "hash": "bafybeiflmm46nr4vv2h3wuzbx3pukcz7ju4fhbfzt6yxmoo533uktlgru4",
  "numberOfEntities": 12345,
  "replacedSnapshotHashes": [ "bafybeicw6x75ieaxfwynekbyhpcsgctpjkt6cb4j6oa7s57qjj6e4b5phd" ],
  "timeRange": {
     "initTimestamp": 1684281600000,
     "endTimestamp": 1684886400000
  }
}
```

### 下载快照 <a href="#download" id="download"></a>

使用 `hash` 快照的该字段，客户端可以下载在该时间范围内创建或更新的相关实体。

快照文件以这一行精确开头：

```
### Decentraland JSON 快照
```

之后，每一行都是一个描述 [实体](/docs/contributor/contributor-zh/nei-rong/entities/) 的 JSON 文档，包含以下字段：

| 字段                | 值                                                                              |
| ----------------- | ------------------------------------------------------------------------------ |
| `entityId`        | 此 [实体](/docs/contributor/contributor-zh/nei-rong/entities/).                      |
| `entityType`      | 以下之一 `场景`, `资料`, `可穿戴物品`, `表情动作`, `商店` 或 `装扮`.                                 |
| `指针`              | 一个 [指针](/docs/contributor/contributor-zh/nei-rong/pointers/) 会解析到（或曾解析到）该实体的指针数组。 |
| `entityTimestamp` | 此实体上传时的 Unix UTC 时间戳（毫秒）。                                                      |
| `authChain`       | 该 [身份验证链](/docs/contributor/contributor-zh/nei-rong/entities/#ownership) 用于此实体。   |

典型条目如下：

```json
{
  "entityId": "bafkreigrvaqynmiglpvewwhn2yd63q5dvagrrt5jbhimzvbrn5kimj5zne",
  "entityType": "wearable",
  "pointers": ["urn:decentraland:matic:collections-v2:0x11a6879861f36cbad632a4e7226816a16139fb33:0"],
  "entityTimestamp": 1671117456129,
  "authChain": [
    // ... 认证链载荷和签名
  ]
}
```


> [!NOTE]
> 如果你打算逐行解析快照，请记得跳过（或者更好地，验证）带有标题的第一行，并准备处理文件末尾的空行。


#### 开始构建实体索引 <a href="#index-start" id="index-start"></a>

希望为整个活动实体集合建立索引的客户端，应处理当前所有可用的快照，并保留最新的 [实体](/docs/contributor/contributor-zh/nei-rong/entities/) 针对每个 [指针](/docs/contributor/contributor-zh/nei-rong/pointers/).

最简单的策略是按时间倒序处理快照（即最新的先处理），忽略已经发现过的指针，以保留对最新实体的引用。

伪代码如下：

```py
# 下载当前的快照集合，并按从新到旧排序：
snapshots = get_snapshots()
snapshots.sort('timeRange.initTimestamp', DESCENDING)

seen_pointers = set()

# 处理快照，为每个指针保留最新的实体：
for snapshot in snapshots:
    items = get_snapshot_items(snapshot) 

    for item in items:
        if any(pointer in seen_pointers for pointer in item.pointers):
            discard(item)
        else:
            keep(item)
            seen_pointers.update(item.pointers)
```

由于单个实体可能会被多个指针引用（这种情况通常见于 [场景](/docs/contributor/contributor-zh/nei-rong/shi-ti-lei-xing/scenes/)），因此在决定保留还是丢弃该项之前，必须检查它们全部。


> [!NOTE]
> 用于更长时间范围的快照文件可能非常大。对于不需要为整个实体集建立索引的开发和试验用途，建议使用较小的快照。最终得到的实体集合将是不完整的，但仍然有效。


#### 更新实体索引 <a href="#index-update" id="index-update"></a>

维护最新实体索引的客户端可以定期调用 [`快照`](https://decentraland.github.io/catalyst-api-specs/#tag/Content-Server/operation/getSnapshots) 端点，并通过考虑以下因素来决定是否下载每个文件：

* 由以下标识的快照是否 `hash` 已下载？
* 是否 `hash` 在 `replacedSnapshotHashes` 已下载的另一份快照的列表中？
* 该 `timeRange` 与当前目的相关？

如果需要处理任何新快照，可以使用上面的相同策略来更新现有数据集。

### 示例

在 [练习](https://github.com/decentraland/docs/tree/main/contributor/content/practice/README.md) 部分，你会找到可与快照系统配合使用的代码示例。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-zh/nei-rong/snapshots.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
