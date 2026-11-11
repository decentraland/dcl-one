> 搜索持久化的多人服务器日志，跟踪实时日志，并检查场景性能指标。

# Scene Insights

Scene Insights 可帮助你调查已部署的 [多人服务器](/docs/creator/content-creator-zh/chang-jing-sdk7/lian-wang/authoritative-servers/) 场景。搜索历史服务器日志，实时跟踪新消息，并可从你的终端或 AI 助手获取 CPU、内存和其他指标。

**日志和指标会保留 7 天**，并会跨服务器重启和场景重新部署保留。这涵盖服务器端执行；请使用 [生产环境调试](/docs/creator/content-creator-zh/chang-jing-sdk7/tiao-shi/debug-in-prod/) 来排查客户端和渲染问题。

## 更新你的项目

在你的场景项目中运行这些命令以安装最新的 `auth-server` SDK 和运行时：

```bash
npm install @dcl/sdk@auth-server @dcl/js-runtime@auth-server
```

如果你的项目中还列出了 `@dcl/sdk-commands` 直接在 `package.json`中，也请将其更新为 `@auth-server` 。还要确认已安装的 CLI 包含 `登录`, `日志`, `跟踪`, `指标`，以及 `执行记录`:

```bash
npx sdk-commands scene-insights --help
```

对于 AI 辅助调试，还要安装或刷新 [SDK 技能](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/vibe-coding/):

```bash
npx skills add decentraland/sdk-skills --all
```

你可以让你的 AI 助手来完成设置：

> 更新这个场景到最新的 auth-server SDK 和运行时，并更新 Decentraland SDK 技能。帮助我登录 Scene Insights，然后调查过去两小时的服务器错误以及 CPU 和内存使用情况。

读取已部署场景的遥测数据不需要重新部署。权限变更则需要。

## 谁可以读取数据？

使用满足以下任一条件的钱包登录：

* 它出现在已部署场景根级的 `logsPermissions` 数组中。
* 它拥有该 World。
* 它是 World 部署协作者，且其授权覆盖整个 World 或该场景的每个已部署地块。
* 对于 Genesis City，它拥有该场景的 **基础地块**，或者在该地块上拥有已委托的 LAND 管理权限： `操作员`, `updateOperator`, `updateManager`，或 `approvedForAll`.

所有者和符合条件的管理者不需要 `logsPermissions` 条目或重新部署场景。若要明确授予另一个钱包访问权限，请将其添加到 `scene.json` 并重新部署：

```json
{
  "logsPermissions": ["0xYourWalletAddress"]
}
```

对于 Genesis City，即使你查询的是场景中的另一个地块，也会针对已部署场景的基础地块检查权限。仅拥有或管理其他地块并不会授予对该场景的访问权限。

相同的权限适用于历史日志、实时日志、指标和执行历史。访问遵循 **当前权限**：当前获得授权的任何人都可以在同一场景密钥下读取最多 7 天内保留的所有历史记录，包括由其他人部署该场景时写入的日志。这包括新的所有者、操作员、符合条件的租用者，以及新添加到 `logsPermissions`的钱包。在授予访问权限前，请考虑这些历史记录。

权限变更不会立即生效。默认情况下，权限缓存保留 60 秒，打开的实时流每 5 分钟重新检查一次访问权限。对于一个已打开的流，大约需要 6 分钟才能观察到撤销。这些是默认时长，不是严格截止时间：配置以及临时容量故障期间的有限宽限期可能会延长延迟。登录并不会授予读取其他场景的权限。

## 登录

```bash
npx sdk-commands scene-insights login
```

请在浏览器中使用普通钱包或智能合约钱包批准登录。这将创建一个仅限于读取日志、指标和执行记录的 Insights 会话。其有效期最长为 **12 小时**，并受父会话到期时间限制。后续命令会复用它，无需再次批准。

使用以下命令检查已登录地址和过期时间：

```bash
npx sdk-commands scene-insights session --json
```

会话过期后，请运行 `登录` 再次登录。如果 AI 助手正在为你调查，请你亲自批准钱包登录。不要与其共享私钥或缓存的会话文件。

### 保持会话文件私密


> [!WARNING]
> **绝不要提交、部署或共享你的登录会话文件。** 默认保存在 `~/.dcl/scene-insights-session.json`。它包含一个临时私有签名密钥以及你钱包的只读委托。任何拿到副本的人都可以使用这些凭据，在会话到期前读取你的钱包可访问的场景的日志、指标和执行历史。不要把它发送给队友、AI 助手、支持聊天或公开问题报告，也不要将其包含在项目归档中。


SDK 会自动保护它：

* 默认情况下，它会将文件保存在你的主目录下，位于场景项目之外。如果你设置了 `DCL_SCENE_INSIGHTS_SESSION_DIR`，它就会使用该目录下的 `.dcl` 文件夹。
* 它会将 `*` 添加到会话文件夹的 `.gitignore` 中，并拒绝保存或使用 Git 已经跟踪的会话文件。
* 它会以仅所有者权限（`0600`）并在其文件夹上使用 `0700` 支持该功能的系统上的权限创建。
* 标准 SDK 部署文件收集器会排除以点开头的文件和目录，包括 `.dcl`，与 `.dclignore`.

请记住这些检查：

* 在提交前检查暂存文件。不要强制添加会话文件； `.gitignore` 无法保护已被 Git 跟踪的文件。
* 如果你自定义了会话位置，尽可能将其放在场景项目之外。切勿将会话复制或重命名到场景资产或其他可部署文件夹中。
* 如果你使用自定义部署、备份或 ZIP 脚本，请显式排除会话文件和 `.dcl` 凭据；这些工具可能不会应用 SDK 的排除规则。
* 仅共享来自 `npx sdk-commands scene-insights session --json` 在需要时的安全状态输出，绝不要共享会话文件本身。团队成员应使用各自获授权的钱包登录。

`npx sdk-commands scene-insights logout` 会删除本地保存的会话。它 **不会** 撤销已共享的副本：这些凭据在到期前仍可能可用。如果该文件已泄露，请视情况将其从共享制品和 Git 历史中移除；删除本地文件或重新登录并不会使已泄露的副本失效。

## 选择一个场景

在场景项目中，命令会从以下内容推断 World： `scene.json`的 `worldConfiguration.name`，或者使用 `scene.base` 用于 Genesis City。你也可以显式选择：

```bash
# 一个 World
npx sdk-commands scene-insights logs --world myworld.dcl.eth

# 多场景 World 中的一个场景
npx sdk-commands scene-insights logs --world myworld.dcl.eth --position 10,10

# 一个 Genesis City 场景：坐标为负数时使用 =
npx sdk-commands scene-insights logs --position=-125,-96
```

命令默认指向生产环境（`.org`）。对于开发网络上的场景，请添加 `--target https://multiplayer-scenes-insights-service.decentraland.zone` 用于登录、会话检查和查询。整个过程中使用相同的目标还能确保智能合约钱包在正确的网络上验证。

## 搜索日志和执行记录

```bash
# 最近 6 小时内的最新 200 条错误
npx sdk-commands scene-insights logs --from 6h --level error --limit 200

# 在固定的 UTC 时间窗口中查找文本
npx sdk-commands scene-insights logs --from 2026-11-01T14:00:00Z --to 2026-11-01T14:30:00Z --query "player joined"

# 列出过去一天内的服务器运行
npx sdk-commands scene-insights executions --from 1d

# 来自一次运行的日志：将 EXECUTION_ID 替换为 executions 中的某个 ID
npx sdk-commands scene-insights logs --execution EXECUTION_ID --from 1d
```

`日志` 默认显示最近一小时内按时间顺序排列的最新 100 条匹配行。CLI 接受 `--limit` 介于 1 到 10,000 之间的值，并在需要时获取多个 API 页面。 `--level` 是最低严重级别（`warn` ，包含错误）； `--query` 以 ASCII 不区分大小写方式搜索字面文本。使用 `--source scene,engine,stderr` 来选择日志来源，或传入这些来源的子集。

`executionId` 标识一次服务器运行； `entityId` 标识已部署的场景版本。新的执行可能意味着空闲停止/启动、滚动发布或崩溃。在断定其崩溃前，请检查前后日志。

添加 `--json` 以获得机器可读的输出。CLI 会将历史日志作为一个 JSON 文档返回；其中的 `more: true` 字段表示在返回限制之外还有更多匹配项。请缩小时间窗口或增加限制。直接使用 API 的客户端每页最多接收 1,000 行，并使用游标分页。

## 跟踪实时日志

```bash
npx sdk-commands scene-insights tail
```

按 **Ctrl+C** 以停止。若要进行有界捕获，包括在让 AI 助手调查时：

```bash
npx sdk-commands scene-insights tail --for 30s --lines 20 --json
```

这会重放最新的 20 行，然后继续跟踪新的行 30 秒。JSON 输出每行包含一条日志记录。CLI 会在运行中的跟踪期间重新连接并继续。日志通常会在场景写入后几秒内到达。

`跟踪` 支持级别和文本筛选器。对于历史时间窗口或执行/来源筛选器，请使用 `日志`.

如果你使用 `npm run server-logs`，请检查你的项目的 `package.json` 是否包含：

```json
{
  "scripts": {
    "server-logs": "sdk-commands scene-insights tail"
  }
}
```

使用更新后的 SDK 进行完整项目构建时，会创建或迁移 SDK 生成的默认脚本。自定义脚本会被保留；使用 `--skip-build` 和单文件构建不会更新它。你可以直接编辑该脚本，而无需仅为了查看日志而重新构建。

## 检查性能指标

先列出有数据的指标，再查询某个序列：

```bash
npx sdk-commands scene-insights metrics --json
npx sdk-commands scene-insights metrics cpu.percent --from 2h --step 60 --json
npx sdk-commands scene-insights metrics mem.heap_used --from 2h --step 60 --json
```

`--step` 设置桶大小（秒）。可用指标取决于引擎版本和场景活动情况。指标发现和 `执行记录` 默认显示最近 24 小时。使用 `--execution EXECUTION_ID` 用于指标序列以隔离一次运行。

| 指标                                                      | 它告诉你什么                                           |
| ------------------------------------------------------- | ------------------------------------------------ |
| `cpu.percent`                                           | 执行线程 CPU 占单个核心的百分比；持续接近 100% 的值表明该线程很忙           |
| `cpu.run_ms`                                            | 在以下内容中花费的 CPU 时间： `onStart`/`onUpdate`，而不是实际时钟延迟 |
| `mem.heap_used`, `mem.heap_limit`, `mem.external_bytes` | JavaScript 堆使用量、堆上限和 V8 外部内存                     |
| `logs.lines`, `logs.bytes`                              | 在速率限制前的控制台输出；有助于发现过度日志记录                         |
| `fetch.failed`, `storage.failed`                        | 用于出站 HTTP 或存储调用的请求发送错误；可与错误日志关联                  |
| `fetch.status_4xx`, `fetch.status_5xx`                  | HTTP 错误响应；这些不计入 `fetch.failed`                   |
| `logs.dropped`                                          | 由引擎每个 tick 的日志预算丢弃的行                             |
| `comms.msgs_out`, `comms.bytes_out`                     | 发送给玩家的消息和字节数                                     |

CPU 指标并不衡量整个服务器，内存指标也不是进程总内存或容器上限。在诊断泄漏或性能问题之前，请比较相同的执行、时间窗口和工作负载。

仪表值返回 `平均值` 和 `最大值` ，每个桶一个值。求和返回 `总和` 以及每秒的 `速率`。 `null` 点表示没有存储的样本，而不是使用量为零；空闲场景、延迟和收集失败都可能造成缺口。

## 保留与限制

| 限制              | 默认值                                            |
| --------------- | ---------------------------------------------- |
| 日志和指标保留期        | 7 天                                            |
| 最大查询时间范围        | 7 天                                            |
| 每次 CLI 查询的日志结果数 | 默认 100 条；通过分页最多可达 10,000 条                     |
| 指标序列            | 最多 1,500 个桶；增大 `--step` 以适配更长的窗口               |
| 持续场景日志记录        | 每秒 50 行和每秒 64 KiB                              |
| 日志突发额度          | 500 行和 512 KiB                                 |
| 每日场景日志配额        | 每个服务器进程 500,000 行或 128 MiB 的软上限；在 UTC 00:00 重置 |

每日配额计数器保存在内存中。服务器进程重启或场景迁移到另一个副本也会重置其计数器，因此每日配额只是跨此类变更的近似上限。

速率限制和每日配额会丢弃超额日志；被丢弃的行之后无法恢复。引擎警告消息会报告丢弃情况和配额重置。达到每日配额后，在重置前不会再持久化更多日志；场景会继续运行。减少重复的每帧日志记录，以便有用消息能落入预算内。

引擎还有一个单独的每个 tick 日志预算（默认 1,000 行）；超出的行会在导出前被丢弃，并计入 `logs.dropped`.

较大的消息可能会被截断（API 默认每条消息 8,192 个字符）。收集属于尽力而为：服务中断、导出失败和速率限制都可能留下缺口。只能获取已收集的遥测数据；安装新的 CLI 无法恢复更早或已丢弃的数据。仅凭空结果并不能证明一个场景是健康的。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/tiao-shi/scene-insights.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
