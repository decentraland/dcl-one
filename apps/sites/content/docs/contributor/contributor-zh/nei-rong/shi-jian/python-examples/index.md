# Python 示例

本练习展示了如何编写一个简单程序，使用 [快照](/docs/contributor/contributor-zh/nei-rong/snapshots/) 内容服务器提供的。


> [!NOTE]
> 你可以找到 [完整脚本](https://github.com/decentraland/documentation/blob/main/content/contributor/content/practice/snapshots_mini.py) 在 GitHub 上，以及一个 [更高级的示例](https://github.com/decentraland/documentation/blob/main/content/contributor/content/practice/snapshots.py).


我们将使用 Decentraland Foundation 的服务器，地址是 `peer.decentraland.org`，并将 Python 3 作为我们的首选语言。

我们要做的是：

1. 查询内容服务器的状态。
2. 选择并下载一个 [快照](/docs/contributor/contributor-zh/nei-rong/snapshots/) ，其中包含一个实体列表。
3. 打印所有引用实体的类型和 ID。

让我们先做一些准备工作开始脚本。我们只使用标准库模块，但在实际代码中，你可能会想要一个更方便的 HTTP 客户端（比如 [requests](https://github.com/psf/requests) 库）。

```py
# 发起一个 HTTP GET 请求，返回一个类文件的 HTTP 响应。
def fetch(path):
    url = f"https://peer.decentraland.org/{path}"
    headers = { "User-Agent": "urllib" } # 在某些服务器上很重要（如果为空，会返回 403 Forbidden）

    request = urllib.request.Request(url, headers=headers)
    response = urllib.request.urlopen(request)

    return response
```

我们的简单辅助函数会发起一个 HTTP `GET` 请求，并返回类文件的响应对象。没什么花哨的。让我们用它来访问 `/about` 端点并检查服务器状态：

```py
# 检查服务器状态：
about = json.load(fetch('about'))

if not about['healthy']:
    print("服务器不健康！")
    sys.exit(1)
```

如果我们通过了这一步，说明服务器正在运行（我们得到了一个 `200` 响应）并报告其运行正常。我们可以请求当前的快照集合（它以 JSON 数组格式返回）：

```py
# 获取快照列表：
all_snapshots = json.load(fetch('content/snapshots'))
```

快照文件（尤其是更长时间范围的）可能非常大。为了快速试验，我们先按 `numberOfEntities`:

```py
# 选择实体数最少的快照：
snapshot = min(all_snapshots, key=lambda s: s['numberOfEntities'])
```

要下载内容，我们需要 `哈希` 字段 `快照`。我们通过将它追加到内容根路径来获取文件 URL：

```py
# 从内容 API 请求文件：
response = fetch('content/contents/' + snapshot['hash'])
```

我们选中的文件足够小，可以缓冲在内存中，但先假设我们不知道这一点，并将其流式读取。第一行是快照头，之后的每一行都包含一个 JSON 对象。

先检查头部总是个好主意：

```py
# 验证快照头：
header = response.readline().decode('utf-8').strip()

if header != '### Decentraland json snapshot':
    print("无效的快照头：" + header)
    sys.exit(1)
```

现在我们可以逐行读取响应来处理快照中的所有实体。就我们这点小用途而言， *process* 表示打印实体类型和 ID：

```py
# 逐行读取并解码所有项目，每行一个 JSON：
for line in response:
    item = json.loads(line)
    print(item['entityType'], item['entityId'])
```

这个循环会开始流式传输、解析并打印类似这样的行，直到快照处理完毕：

```
profile bafkreic36qmzyprs6whkpuxbeiif4no6kvdrr2tfpichbx2fzfz5py6eyv
scene bafkreibr5xfujqrp5q3o4s73vm2yljlcp7cucqgugssnarsuclxv4emlmy
profile bafkreid7khr5wnkialba44rsslffi633rh3lvctad5oa5vjoe6wa7s4c5a
wearable bafkreihlqcb7jgubomyidikpwpqhgzbagltk5m4rgbjdvzydxmoka7bg4i
```

太棒了！我们已经使用快照系统来探索 Decentraland 中一些可用的内容。

记住，你可以找到 [完整脚本](https://github.com/decentraland/documentation/blob/main/content/contributor/content/practice/snapshots_mini.py) 在 GitHub 上，以及一个 [更高级的示例](https://github.com/decentraland/documentation/blob/main/content/contributor/content/practice/snapshots.py).


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-zh/nei-rong/shi-jian/python-examples.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
