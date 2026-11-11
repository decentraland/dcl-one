> 先进行记忆配对，然后触发式怪物追逐，并包含收集进度、排行榜和自定义位图文本。

# 怪物侦察

![Monster Recon 预览](/docs/_remote/276e8747b4e8b433f9cfa9f6.webp)

一款配对记忆游戏：你要在限时棋盘上清除牌面，然后在你揭露的怪物逃跑前追上它。将稀有发现收集到你的图鉴中，并冲击每周和历史排行榜。

* [在 Decentraland 上游玩](https://decentraland.org/jump?realm=monsterrecon.dcl.eth)

## 项目信息

**维护者：** 社区 · carlosmu

[查看上游源代码](https://github.com/carlosmu/dcl-monster-recon)

**许可证：** 在此目录中未标识。重复使用前，请查阅上游仓库和维护者以了解适用条款。


> [!WARNING]
> **社区维护的代码——使用前请审查**
> 
> 此社区项目由 Decentraland Foundation 和 DAO 的 GitHub 组织之外的团队维护。其源代码和线上部署可能会独立变更。
> 
> 收录于此目录并不意味着已进行安全审计或保证其安全性。在运行或重复使用之前，请审查代码、依赖项和安装脚本。
> 
> [报告目录问题](https://github.com/decentraland/docs/issues/new)。对于疑似漏洞，请遵循上游项目的安全政策。不要在公开 issue 中包含利用细节或凭据。


## 主要功能

* 多人服务器
* 用于存储进度、最佳时间、排行榜和每周奖励。
* 虚拟摄像机
* 场景表情动作
* 触发区域
* 粒子
* 补间动画
* TextShapes
* 3D 排行榜（含玩家头像）
* 用于 UI 的自定义位图字体
* 同步显示其他玩家捕获情况的实时信息流

## 探索代码

这些链接指向特定的源代码快照，因此即使上游持续开发，这些说明仍然有用。它们是学习参考，不是安全评估。

### 使用场景标记和触发区域构建追逐流程

`spawnHop` 在所选标记处将上下浮动补间、球形触发器、声音和广告牌精灵组合在一起。捕获处理器会先检查进入的实体是否为本地玩家，然后再清理并调用奖励回调。

**项目上下文：** 请结合提供给以下内容的标记名称、精灵图集和回调一起阅读： `startPrizeChase`.

**源代码参考：**

* [`spawnHop`](https://github.com/carlosmu/dcl-monster-recon/blob/eb0bc6fa87b9ebf98c8c068ebb9bba686a9467cb/src/prizeChase.ts#L216-L271)
* [捕获处理器](https://github.com/carlosmu/dcl-monster-recon/blob/eb0bc6fa87b9ebf98c8c068ebb9bba686a9467cb/src/prizeChase.ts#L183-L214)

### 在 ReactEcs 中渲染自定义位图文本

`BitmapTextLine` 使用图集 UV、字形偏移和字偶距来排布单个字形图像。前面的测量和换行辅助函数会按测量宽度将文本分成多行。

**项目上下文：** 渲染器需要匹配的位图字体度量和纹理资源。

**源代码参考：**

* [`BitmapTextLine`](https://github.com/carlosmu/dcl-monster-recon/blob/eb0bc6fa87b9ebf98c8c068ebb9bba686a9467cb/src/bitmapFont.tsx#L268-L327)
* [测量和换行辅助函数](https://github.com/carlosmu/dcl-monster-recon/blob/eb0bc6fa87b9ebf98c8c068ebb9bba686a9467cb/src/bitmapFont.tsx#L229-L266)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/jiao-cheng-yu-shi-li/published-games/monster-recon.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
