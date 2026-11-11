# 如何运行 Catalyst 节点

你是否曾好奇 Decentraland Catalyst 节点是如何工作的？是否曾想自己运行一个节点，却不知道从哪里开始而感到无从下手？你是否正在为 Decentraland 构建内容，并发现要在生产服务器上完成整个开发和测试流程很困难？

如果你正是（或者曾经有过这种情况），别再担心。本教程将帮助你规划过程，弄清楚你需要决定什么，然后逐步指导你启动服务器。另外，如果事情没有按计划进行，末尾还有一个故障排除部分，列出了一些最常见的问题及其解决方法。

准备好了吗？开始动手吧。

### 硬件要求

Decentraland Foundation 的服务器部署在 Amazon AWS 上，运行于 `t2.xlarge` 实例上。因此，尽管这些服务器用于公开访问，你自己的节点可能只需要更小的规格，具体取决于其预期用途。

一个 AWS EC2 `t2.xlarge` 具有以下硬件：

* 4 个 vCPU。
* 16 GB RAM

至于文件存储 + 数据库存储，容量为 2 TB 的 SSD / HDD 应该绰绰有余。撰写本文时，foundation 服务器的所有存储总共只用了略多于 1 TB。

### 软件前置要求

运行 catalyst-owner 需要以下内容：

* Linux / MacOS 操作系统
* Docker / docker-compose
* Git
* LiveKit 集群

### 在开始之前，需要考虑的一些选项

* 你希望你的节点公开暴露在互联网上吗？如果是，你需要一个可从互联网访问的实例，以及一个可用于连接它的公共 URL。
* 这个节点会用于场景、穿戴品等内容的开发吗？在这种情况下，硬件规格可以小得多，因为访问它的可能只有一两个人，而不是数百名用户。
* 你想同步所有实体类型吗？如果你是用它来开发场景，可能最好跳过 profiles 的同步，因为它们会消耗大量时间、带宽，最重要的是磁盘空间，而且对你的目标并没有帮助。

### LiveKit

Catalyst 的 Comms 服务需要一个外部 LiveKit 集群来协调玩家之间的通信。为此有两个选项：使用 LiveKit Cloud 账户，或者运行你自己的 LiveKit 集群。

建议设置 [LiveKit Cloud 账户](https://cloud.livekit.io/) 它提供一个可管理 100 名用户的免费套餐，或者也可以根据流量付费。这种方式更简单，因为不需要额外部署基础设施，而且服务本身可以负责扩缩容。否则，就需要自行部署 LiveKit 集群。LiveKit 的文档写得非常好：

* [部署](https://docs.livekit.io/oss/deployment/)
* [分布式设置](https://docs.livekit.io/oss/deployment/distributed/)

*如果你想深入了解支持基于 LiveKit 传输的通信所需的技术细节，请查看* [*ADR-70 新通信架构*](https://adr.decentraland.org/adr/ADR-70)*.*

### 分步指南

首先要做的是克隆 [catalyst-owner](https://github.com/decentraland/catalyst-owner) GitHub 上的仓库

```bash
> git clone https://github.com/decentraland/catalyst-owner.git
正在克隆到 'catalyst-owner'...
远程：正在枚举对象：2392，完成。
远程：正在计数对象：100% (602/602)，完成。
远程：正在压缩对象：100% (168/168)，完成。
远程：共 2392 个（delta 506），已复用 481 个（delta 430），重新打包复用 1790 个
正在接收对象：100% (2392/2392)，926.05 KiB | 1.09 MiB/s，完成。
正在解析 deltas：100% (1257/1257)，完成。
> cd catalyst-owner
```

完成后，就该为节点配置环境变量，并根据需要进行修改。

```bash
# 在 /catalyst-owner 文件夹中
> cp .env.example .env
> cp .env-advanced.example .env-advanced
```

现在用你喜欢的文本编辑器，对这些文件做出必要的修改。例如，设置 `EMAIL` 带有效邮箱的环境变量是必需的，这样你才能收到有关 Certbot 证书过期的更新。

另外，还需要为以下项设置你自己的具体值： `CATALYST_URL` 这可能是必需的，尤其是当你的服务器要公开暴露在互联网上时。在本地机器上使用时，默认值 `http://localhost` 就可以。

**Comms 服务**

一旦 LiveKit 集群可用，你需要为 Comms 服务设置特定的 LiveKit 变量，例：

```
LIVEKIT_HOST=wss://livekit-1.mydomain.org
LIVEKIT_API_KEY=API-JjHuvM
LIVEKIT_API_SECRET=J7YSHmNzkNCEfT2
ROOM_PREFIX=my-prefix
```

该 `ROOM_PREFIX` 变量是可选的，可用于标识由 Catalyst 创建的 LiveKit 房间。如果有多个 Catalyst 使用同一个 LiveKit 集群，这会很有用。

**内容服务器**

有一个名为 `CONTENT_SERVER_STORAGE` 的变量，它定义内容服务器用于存储的本地文件夹。其默认值为 `CONTENT_SERVER_STORAGE=./storage`。你可以更改这个值，把文件存放到其他位置，或者至少确保所引用的文件夹存在。你可能需要像这样创建它：

```bash
# 在 /catalyst-owner 文件夹中，或者在 CONTENT_SERVER_STORAGE 指向的任何位置
> mkdir storage
```

还有另一个有趣的变量 `SYNC_IGNORED_ENTITY_TYPES` 它允许在同步过程中忽略某些实体类型。如果你要将 Catalyst 服务器用于开发，也许并不需要同步所有内容类型。你可以设置环境变量 `SYNC_IGNORED_ENTITY_TYPES="profile,store"` 这样就只会从 DAO 服务器获取场景和穿戴品。这将为你的服务器节省大量时间、带宽和磁盘存储。

> 💡 如需查看受支持环境变量的更完整列表，请看一下 [环境变量](#environment-variables) 下面这一节。

#### 启动 Catalyst 节点

当所有环境变量都配置好后，就该启动节点了。只需运行 `init.sh` 根目录下的脚本即可。

```bash
# 在 /catalyst-owner 文件夹中

> ./init.sh
## 正在加载环境变量... [ OK ]
[ OK ]
## 检查邮箱是否已配置... [ OK ]
## 检查存储是否已配置... [ OK ]
## 检查 catalyst URL 是否已配置... [ OK ]
 警告：你运行的不是 Catalyst Content 和 Catalyst Lambdas 节点的最新镜像。
 警告：你运行的不是 Catalyst Archipelago 节点的最新镜像。
 警告：你运行的不是 Catalyst Explorer BFF 节点的最新镜像。
 - DOCKER_TAG:               45a5154f11b53d55aadfdf7f958e2fce6a964824
 - LIGHTHOUSE_DOCKER_TAG:    latest
 - CATALYST_URL:             http://localhost
 - CONTENT_SERVER_STORAGE:   ./storage
 - EMAIL:                    a@a.com
 - ETH_NETWORK:              mainnet
 - REGENERATE:               0

5 秒后启动...
45a5154f11b53d55aadfdf7f958e2fce6a964824：正在从 decentraland/catalyst-content 拉取
摘要：sha256:d6c2981c57cb9367fdb3228d15b07e4d26e62c902944115937de24ecd3ab6aac
状态：decentraland/catalyst-content:45a5154f11b53d55aadfdf7f958e2fce6a964824 的镜像已是最新
docker.io/decentraland/catalyst-content:45a5154f11b53d55aadfdf7f958e2fce6a964824
45a5154f11b53d55aadfdf7f958e2fce6a964824：正在从 decentraland/catalyst-lambdas 拉取
摘要：sha256:5605812dd29316afc32e561d2a2ce1311f164f07ef3cd99a1cd60727518636e5
状态：decentraland/catalyst-lambdas:45a5154f11b53d55aadfdf7f958e2fce6a964824 的镜像已是最新
docker.io/decentraland/catalyst-lambdas:45a5154f11b53d55aadfdf7f958e2fce6a964824
latest：正在从 decentraland/catalyst-lighthouse 拉取
摘要：sha256:fedc10b714823909f0a1c17955eed2f22a02e4f784090848d3253ef734e410d4
状态：decentraland/catalyst-lighthouse:latest 的镜像已是最新
docker.io/decentraland/catalyst-lighthouse:latest
latest：正在从 decentraland/archipelago-service 拉取
摘要：sha256:e2c1d6fa96a5cfbbf6fb37e3840c0724fb5abeef8c366025a28fab3ad33d6480
状态：quay.io/decentraland/archipelago-service:latest 的镜像已是最新
quay.io/decentraland/archipelago-service:latest
latest：正在从 decentraland/explorer-bff 拉取
摘要：sha256:fa9e305c44972a8613b01f4cd573d6fb84b0fb03ce953a506fbb06053202913c
状态：quay.io/decentraland/explorer-bff:latest 的镜像已是最新
quay.io/decentraland/explorer-bff:latest
正在停止 nginx ... 完成
## 使用 HTTP，因为 CATALYST_URL 设置为 http://localhost
## 正在替换 nginx 服务器文件中的 $katalyst_host 值... [ OK ]
## 正在重启容器...
使用默认驱动程序创建网络 "catalyst-owner_default"
正在创建 catalyst-owner_comms-server_1 ... 完成
正在创建 node-exporter ... 完成
正在创建 catalyst-owner_lambdas_1 ... 完成
正在创建 postgres ... 完成
正在创建 catalyst-owner_certbot_1 ... 完成
正在创建 nats ... 完成
正在创建 postgres-exporter ... 完成
正在创建 catalyst-owner_content-server_1 ... 完成
正在创建 catalyst-owner_archipelago_1 ... 完成
正在创建 catalyst-owner_explorer-bff_1 ... 完成
正在创建 nats-exporter ... 完成
正在创建 cadvisor ... 完成
正在创建 nginx ... 完成
## Catalyst 服务器已启动并运行于 http://localhost
```

如果一切顺利，我们现在已经运行了一个完整的 Decentraland 节点。你现在可以打开浏览器输入 URL。如果使用默认设置，应该是 [`http://localhost`](http://localhost/).

我们可以使用这个 docker 命令查看内容服务器日志：

```bash
> docker logs catalyst-owner_content-server_1
```

尽管服务器现在已经在运行，但它还没有 100% 准备好投入使用。它需要从其他 [DAO 的服务器](https://decentraland.github.io/catalyst-monitor) 这样它才能为连接到它的用户提供相同的体验。同步可能需要很长时间。即使网络条件不错，6 小时也很常见。所以……去喝杯咖啡，打个盹，睡个好觉，明天再回来吧。

判断同步是否完成的一种方法，是查看 content status 端点的结果： <http://localhost/content/status> （这里用你的 URL）。

```json
{
  "synchronizationStatus": {
    "lastSyncWithDAO": 1658153514917,
    "synchronizationState": "Bootstrapping"
  },
  "snapshot": {
    "entities": {
      "profile": 1271331,
      "scene": 23477,
      "wearable": 17351,
      "store": 890
    },
    "lastUpdatedTime": 1658153414530
  },
  "version": "v3",
  "commitHash": "9a2e0d5d05d02646df2e1e5d00436d3166a07aa1",
  "catalystVersion": "4.8.6",
  "ethNetwork": "mainnet"
}
```

当 `synchronizationState` 为 `Bootstrapping`时，节点将暂时暂停接受新的部署。此举可确保在节点在 DAO 网络中更新到最新之前，不会部署新的实体。一旦状态变为 `Syncing`，就表示节点已经成功追上，并且现在持续接收最新更新。这是节点完全正常工作并接受新部署的健康状态。

```json
{
  "synchronizationStatus": {
    "lastSyncWithDAO": 1658153872301,
    "synchronizationState": "Syncing"
  },
  "snapshot": {
    "entities": {
      "profile": 1271317,
      "scene": 23477,
      "store": 890,
      "wearable": 17350
    },
    "lastUpdatedTime": 1658152050030
  },
  "version": "v3",
  "commitHash": "9a2e0d5d05d02646df2e1e5d00436d3166a07aa1",
  "catalystVersion": "4.8.6",
  "ethNetwork": "mainnet"
}
```

如果你更偏向 CLI 风格，可以在内容服务器 docker 镜像日志中 grep 这条消息： `开始从服务器指针变更同步实体`。一旦你看到这段文字，就表示内容服务器已完全同步并可全面使用。

### 环境变量

以下是内容服务器所有环境变量及其默认值的完整列表，这些内容会在启动时记录到内容服务器日志中：

```jsx
STORAGE_ROOT_FOLDER: "/app/storage/content_server/"
DENYLIST_FILE_NAME: "denylist.txt"
DENYLIST_URLS: "https://config.decentraland.org/denylist"
SYNC_IGNORED_ENTITY_TYPES: ""
FOLDER_MIGRATION_MAX_CONCURRENCY: 1000
SERVER_PORT: 6969
LOG_REQUESTS: false
UPDATE_FROM_DAO_INTERVAL: 1800000
SYNC_WITH_SERVERS_INTERVAL: 45000
CHECK_SYNC_RANGE: 1200000
DECENTRALAND_ADDRESS: "0x1337e0507eb4ab47e08a179573ed4533d9e22a7b"
DEPLOYMENTS_DEFAULT_RATE_LIMIT_TTL: 60
DEPLOYMENTS_DEFAULT_RATE_LIMIT_MAX: 300
ETH_NETWORK: "mainnet"
LOG_LEVEL: "debug"
FETCH_REQUEST_TIMEOUT: "2m"
USE_COMPRESSION_MIDDLEWARE: false
BOOTSTRAP_FROM_SCRATCH: false
REQUEST_TTL_BACKWARDS: 1200000
LAND_MANAGER_SUBGRAPH_URL: "https://api.thegraph.com/subgraphs/name/decentraland/land-manager"
COLLECTIONS_L1_SUBGRAPH_URL: "https://api.thegraph.com/subgraphs/name/decentraland/collections-ethereum-mainnet"
COLLECTIONS_L2_SUBGRAPH_URL: "https://api.thegraph.com/subgraphs/name/decentraland/collections-matic-mainnet"
THIRD_PARTY_REGISTRY_L2_SUBGRAPH_URL: "https://api.thegraph.com/subgraphs/name/decentraland/tpr-matic-mainnet"
BLOCKS_L1_SUBGRAPH_URL: "https://api.thegraph.com/subgraphs/name/decentraland/blocks-ethereum-mainnet"
BLOCKS_L2_SUBGRAPH_URL: "https://api.thegraph.com/subgraphs/name/decentraland/blocks-matic-mainnet"
PSQL_DATABASE: "content"
PSQL_HOST: "postgres"
PSQL_SCHEMA: "public"
PSQL_PORT: "5432"
GARBAGE_COLLECTION: true
GARBAGE_COLLECTION_INTERVAL: 21600000
PG_IDLE_TIMEOUT: 30000
PG_QUERY_TIMEOUT: 60000
PG_STREAM_QUERY_TIMEOUT: 600000
SNAPSHOT_FREQUENCY_IN_MILLISECONDS: 21600000
CUSTOM_DAO: undefined
DISABLE_SYNCHRONIZATION: false
SYNC_STREAM_TIMEOUT: "10m"
CONTENT_SERVER_ADDRESS: "http://localhost/content/"
REPOSITORY_QUEUE_MAX_CONCURRENCY: 50
REPOSITORY_QUEUE_MAX_QUEUED: 300
REPOSITORY_QUEUE_TIMEOUT: "1m"
ENTITIES_CACHE_SIZE: 150000
DEPLOYMENT_RATE_LIMIT_MAX: {}
DEPLOYMENT_RATE_LIMIT_TTL: {}
VALIDATE_API: false
RETRY_FAILED_DEPLOYMENTS_DELAY_TIME: 900000
```

### 将你的节点用于生产环境

如果你想运行自己的服务器并帮助扩展网络，首先这非常棒，社区和基金会都非常感谢你这样做。

在这种情况下，你需要通过访问 [此链接](https://governance.decentraland.org/submit/catalyst/)来向 DAO 申请加入网络的批准。你也可以申请一笔 MANA 资助，以覆盖基础设施和管理费用。

你的硬件规格最好更符合上文硬件要求中的建议，因为该服务器会被社区中的任何成员用来进入 Decentraland。

### API 规格

关于内容、lambdas 和 comms 服务器 API 的更多细节，你可以查看 [API 规格](https://decentraland.github.io/catalyst-api-specs).

### 故障排查/常见问题

在启动服务器时，可能会有一些地方出错。以下是长期以来最常见的一些问题及其解决方法。

#### 5432 端口被本地 postgres 占用

如果你在一台用于开发的机器上运行节点，或者这台机器已经托管了其他服务，那么很可能已经有一个 postgres 数据库服务器在运行。因此 5432 端口很可能已经被占用。

最简单的解决方法是停止本地已经在运行的 postgres，然后使用以下命令重启 postgres docker 容器： `docker start postgres`.

如果你熟悉 docker compose，可以开始调整 `docker-compose.yml` 以及环境变量，尝试让它在其他端口上运行。为了保持这里的简单性，本教程将不走那条路。

#### 80 端口被本地 nginx 占用

同样的情况也会发生在 80 端口（甚至 443 端口）上。如果你在本机运行 nginx，那么 80/443 端口很可能已经绑定给了那个服务，因此 Catalyst Docker 容器将无法使用它们。

同样，最简单的方法是停止本地 nginx 服务，然后使用以下命令重启 docker 容器： `docker start nginx`。或者， `docker-compose.yml` 和 `.env` 你需要借助它让服务在其他端口上运行。

### 供参考的其他文档

如果你想更好地了解 Catalyst 服务器是做什么的、它包含哪些服务等，可以查看 architecture 仓库 <https://github.com/decentraland/architecture，其中详细解释了各个部分。>

如果你想为 Catalyst 服务器代码做贡献，请务必查看 [贡献指南](https://github.com/decentraland/catalyst/blob/main/docs/CONTRIBUTING.md) 以了解我们的开发流程、如何提出 bug 修复和改进建议，以及如何构建和测试你的更改。

最后，如果你需要联系团队，可以通过 [Discord](https://discord.com/channels/417796904760639509/948230185457696820) 或提交 [GitHub 问题](https://github.com/decentraland/catalyst/issues).


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-zh/jiao-cheng/how-to-run-a-catalyst.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
