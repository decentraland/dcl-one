> 使用第三方服务器构建多人 Decentraland 场景。

# 第三方服务器

你的场景中多人交互的推荐方式是使用 [多人服务器](/docs/creator/content-creator-zh/chang-jing-sdk7/lian-wang/authoritative-servers/)。本文档还涵盖了其他同样受支持的选项。

* **REST API + 数据库**：适用于不经常变化的数据（排行榜、留言簿）。玩家轮询 API 以获取更新；状态会在会话之间持久保存。参见 [网络连接](/docs/creator/content-creator-zh/chang-jing-sdk7/lian-wang/network-connections/) ，了解如何从场景中发起 `fetch` 请求。
* **WebSocket 服务器**：支持实时双向通信。参见 [网络连接](/docs/creator/content-creator-zh/chang-jing-sdk7/lian-wang/network-connections/) 以了解 WebSocket 的用法。像 [Colyseus](https://colyseus.io/) 这样的库与 Decentraland SDK 配合得很好。


> [!WARNING]
> **📔 注意**：第三方服务器不会与 `syncEntity`, `validateBeforeChange`，或 `存储` 集成——你需要自己实现状态管理和同步逻辑。


也可以采用混合方案，通过 [serverless-multiplayer](/docs/creator/content-creator-zh/chang-jing-sdk7/lian-wang/serverless-multiplayer/)在玩家之间通知变更，但最终状态也会通过 REST API 保存，以供未来访客查看。

以下是一些使用第三方服务器的场景示例：

* [排行榜](https://github.com/decentraland/sdk7-goerli-plaza/tree/main/leader-board)
* [留言簿](https://github.com/decentraland/sdk7-goerli-plaza/tree/main/guest-book-api)
* [验证真实性](https://github.com/decentraland/sdk7-goerli-plaza/tree/main/validate-player-authenticity)

## 本地测试

要预览一个使用第三方服务器的场景，你必须同时运行该场景和它所依赖的服务器。为了更容易测试，服务器可以在与预览相同的机器上本地运行。在本地运行时，服务器可以使用不安全的 `http` 或 `ws` 连接，以便更轻松地设置。

要启动服务器，请进入 `/server` 文件夹并运行 `npm run start`.

在两个单独的窗口中打开预览，每个窗口都会被视为一个独立的玩家。用不同的地址连接每个窗口。两个客户端都会连接到同一个本地服务器实例。

在 Creator Hub 中，打开 **多实例预览** 在 **桌面客户端** 子菜单中的 **播放选项**，然后第二次点击 **播放** 按钮，以打开第二个 Decentraland explorer 窗口。你必须在两个窗口中使用不同的地址进行连接。当场景重新加载时，这些会话仍会保持打开。

作为替代方案，你可以在浏览器 URL 中输入以下内容来打开第二个 Decentraland explorer 窗口：

> `decentraland://realm=http://127.0.0.1:8000&local-scene=true&debug=true&multi-instance=true`

## 独立领域

Decentraland 中的玩家存在于许多彼此独立的 *realms*中。不同领域中的玩家无法看见彼此、互动或聊天，即使他们站在同一地块上也是如此。这样分隔玩家的方式使 Decentraland 能够处理无限数量的玩家，而不会遇到任何限制。它还会将处于相近区域的玩家配对，以确保交互玩家之间的 ping 时间可接受。

如果你的场景向第三方服务器发送数据，以便实时同步玩家之间的变更，那么确保只在同一领域的玩家之间同步变更非常重要。你应当将属于某一领域的所有变更与其他领域的变更分开处理。否则，玩家会看到事物以一种诡异的方式变化，却没有人去做出这些变化。

参见如何在 [获取玩家数据](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/user-data/)

## 多人持久性

不同于本地场景——玩家每次走入时都会重新挂载——使用第三方服务器的场景，其生命周期远远超出了玩家进入和离开场景的时间。

因此，你必须在设计体验时考虑到：玩家并不总会在相同的初始状态下看到场景。对场景所做的任何更改都会持续保留，供其他玩家发现；你必须确保这些更改不会以不希望的方式干扰未来玩家的体验。

### 重置状态

加载场景时，务必确保它是基于存储在服务器中的共享信息构建的，而不是默认状态。

在某些情况下，在场景中包含某种重置按钮是有意义的。按下重置按钮会让场景优雅地重置。

有时，这只是把场景状态中的变量恢复为默认值。但重置场景也可能涉及取消订阅监听器，以及停止服务器端的循环。如果每次重置场景时都残留空循环，这些循环会不断堆积，并对场景性能造成不良影响。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/lian-wang/third-party-servers.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
