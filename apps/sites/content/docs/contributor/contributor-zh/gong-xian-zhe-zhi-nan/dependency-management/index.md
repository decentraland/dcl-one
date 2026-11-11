# 依赖管理

本文档定义了 Decentraland 生态系统中 JavaScript/TypeScript 项目管理依赖项的推荐实践。它适用于前端、后端、SDK、共享库以及任何基于 npm/yarn/pnpm 的包。

> **简而言之**:
>
> * **版本策略** → 使用 **精确/固定版本** 用于 `依赖项` （安全）以及 `devDependencies` （一致的开发环境）。仅将 **版本范围** 仅用于 `peerDependencies` （灵活性）。
> * **例外** → Decentraland 包（`@dcl/*`, `decentraland-*`）可以使用版本范围（`^`），因为它们是受信任的内部包。
> * **库/共享包** → 使用 `peerDependencies` 共享库（React、@dcl/schemas 等）使用版本范围（^）。使用 `依赖项` 固定版本，仅适用于可安全重复的工具库。
> * **应用/最终服务** → 使用 `依赖项` 对运行时包（React、ethers 等）使用固定版本。

## 1. 原因

许多库依赖全局单例、React Context、类身份、共享缓存、schema 枚举或共享数据库连接池。安装同一库的多个版本可能导致：

* 不同副本之间的 Context 未共享
* 重复的缓存或连接池
* instanceof 检查失败
* 枚举/符号不匹配
* 增加打包体积
* 难以诊断的运行时 bug

正确的依赖管理可以防止这些问题。

### 非目标

本标准不会 **并不是** 试图：

* 强制使用单一包管理器（npm、yarn、pnpm 都受支持）
* 保证在 `node_modules` 中只有一个物理副本（重点是运行时/打包去重）

## 2. 定义

| 字段                     | 目的                                   | 消费者责任                                | 适用于     |
| ---------------------- | ------------------------------------ | ------------------------------------ | ------- |
| `依赖项`                  | 模块内部使用的包                             | 无——不期望消费者提供它们                        | 应用和库    |
| `peerDependencies`     | 运行时必须解析为单一有效版本的包                     | 必须安装它们（或对可选项使用 peerDependenciesMeta） | **仅限库** |
| `peerDependenciesMeta` | peerDependencies 的元数据，用于将 peer 标记为可选 | 无——仅影响包管理器行为                         | **仅限库** |
| `devDependencies`      | 仅在开发期间使用的工具                          | 无                                    | 应用和库    |

### 关于 peerDependenciesMeta

`peerDependenciesMeta` 是一个提供关于你的 `peerDependencies`的元数据的字段。最常见的用例是将 peer 标记为 **可选**:

* **必需的 peer** （默认）：如果未安装，包管理器会警告
* **可选的 peer**：如果缺失，包管理器不会警告；你的包应优雅地处理其缺失

这对那些可以在有或没有某些依赖的情况下工作的包很有用（例如，一个可以在有或没有 React 的情况下工作的工具，或支持多个 Web3 提供者的库）。

### 应用 vs 库

**库 / 共享包：**

* 为简单的 UI 状态使用 `peerDependencies` 对于运行时必须解析为单一有效版本的包（React、ethers、@dcl/schemas）
* 由消费者（应用）提供这些依赖
* 防止重复安装和单例冲突

**应用 / 最终服务：**

* 通常使用 `依赖项` 用于运行时包（React、ethers 等）
* 它们是最终消费者，因此重复不是问题
* `peerDependencies` 如果该应用可能作为依赖被使用，则仍然有效

## 3. 何时使用每个字段

> **快速参考**：参见 [第 4 节](#4-examples) 了解常见包及其推荐放置方式。

### 将 peerDependencies 用于（仅限库）：

**在共享库/包中**，使用 `peerDependencies` 用于运行时必须解析为单一有效版本的包：

* React、Redux、wagmi、ethers、viem
* @dcl/schemas 及类似的跨生态系统库
* decentraland/connect
* 在共享连接池时的数据库驱动
* 任何依赖单例或上下文的库

✅ 正确（库）：

```json
{
  "peerDependencies": {
    "react": "^18.0.0",
    "@dcl/schemas": "^20.0.0"
  }
}
```

❌ 错误（库使用 dependencies 来管理共享库）：

```json
{
  "dependencies": {
    "react": "^18.0.0"
  }
}
```

### 将 dependencies 用于：

* 可安全重复的工具库（lodash-es、date-fns）
* **在应用中**：运行时包，如 React、ethers（当应用是最终消费者时）

> **重要**：始终使用 **精确/固定版本** 中的 `依赖项` 固定版本以确保安全。

✅ 正确（库）：

```json
{
  "dependencies": {
    "lodash-es": "4.17.21",
    "date-fns": "3.6.0"
  }
}
```

✅ 正确（应用）：

```json
{
  "dependencies": {
    "react": "18.3.1",
    "ethers": "6.13.0",
    "lodash-es": "4.17.21"
  }
}
```

### 可选 peerDependencies（peerDependenciesMeta）

对于可以在有或没有某些依赖的情况下工作的可复用包，使用 `peerDependenciesMeta` 将 peer 标记为可选：

✅ 正确：

```json
{
  "peerDependencies": {
    "react": "^18.0.0",
    "ethers": "^6.0.0"
  },
  "peerDependenciesMeta": {
    "ethers": {
      "optional": true
    }
  }
}
```

**这意味着：**

* `react` 为 **必需**：消费者必须安装它，否则包管理器会警告
* `ethers` 为 **可选**：消费者不需要安装它；你的包在使用前应检查其是否存在

**使用场景：**

* 可在 React 和非 React 环境中工作的包
* 支持多个 Web3 提供者（ethers、viem 等）的库
* 增强其他库但并非必需的工具库

### 将 devDependencies 用于：

* 工具（TypeScript、ESLint、测试器、打包器）

> **重要**：始终使用 **精确/固定版本** 中的 `devDependencies` 以确保整个团队拥有一致的开发环境。

✅ 正确：

```json
{
  "devDependencies": {
    "typescript": "5.4.5",
    "eslint": "8.57.0",
    "vitest": "1.6.0"
  }
}
```

## 4. 示例

### 必须作为 peerDependencies 的包（共享上下文 / 单例）

* `react`, `react-dom`, `react-redux`, `react-router-dom`
* `redux`, `@reduxjs/toolkit`
* `ethers`, `viem`, `wagmi`
* `@dcl/schemas`, `@dcl/ui-env`, `@dcl/crypto`
* `decentraland-dapps`, `decentraland-ui`, `decentraland-ui2`, `decentraland-connect`
* `pg`, `pg-pool`

### 应作为 dependencies 的包（可安全重复）

* `lodash-es`, `date-fns`
* `uuid`, `nanoid`
* `zod`, `ajv`
* `ms`, `mitt`, `fp-future`

## 5. 相关标准

* [测试标准](/docs/contributor/contributor-zh/gong-xian-zhe-zhi-nan/testing-standards/)
* [UI 标准](https://github.com/decentraland/docs/tree/main/contributor/contributor-guides/ui-standards/README.md)
* [Web UI 标准](https://github.com/decentraland/docs/tree/main/contributor/contributor-guides/web-ui-standards/README.md)
* [WKC 架构](https://github.com/decentraland/docs/tree/main/contributor/contributor-guides/well-known-components/README.md)
* [API 文档指南](/docs/contributor/contributor-zh/gong-xian-zhe-zhi-nan/api-documentation/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-zh/gong-xian-zhe-zhi-nan/dependency-management.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
