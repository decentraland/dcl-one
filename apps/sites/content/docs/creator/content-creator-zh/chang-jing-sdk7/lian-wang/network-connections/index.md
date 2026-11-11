> 如何让你的场景与外部服务器和 API 通信。

# 网络连接

你的场景可以利用提供 API 的外部服务，你可以借此获取最新的价格数据、天气数据或 API 提供的任何其他信息。

你还可以搭建自己的外部服务器来辅助你的场景，并用于在玩家之间同步数据。你可以通过提供 REST API 的服务器来实现，也可以通过使用 WebSockets 的服务器来实现。

## 调用 REST API

你的场景代码可以向 REST API 发送请求以获取数据。

由于服务器可能需要一些时间才能发送响应，你必须将此命令作为一个 [异步函数](/docs/creator/content-creator-zh/chang-jing-sdk7/bian-cheng-mo-shi/async-functions/)，使用 `executeTask()`.

```ts
executeTask(async () => {
	try {
		let response = await fetch(callUrl)
		let json = await response.json()
		console.log(json)
	} catch {
		console.log('无法访问 URL')
	}
})
```

fetch 命令还可以包含第二个可选参数，将标头、HTTP 方法和 HTTP 正文打包到一个对象中。

* **url**: 要发送请求的地址
* **init**: 一个 `RequestInit` 对象，可能包含：
  * **method** : 要使用的 HTTP 方法（GET、POST、DELETE 等）
  * **body**: 请求正文的内容。它必须作为字符串化的 JSON 对象发送。
  * **headers**: 要包含在请求中的附加标头。
  * **redirect**: 重定向策略（'follow' | 'error' | 'manual'）
  * **timeout**: 在请求失败之前等待响应的时长。默认值为 30000 毫秒（30 秒）。

```ts
executeTask(async () => {
	try {
		let response = await fetch(callUrl, {
			headers: { 'Content-Type': 'application/json' },
			method: 'POST',
			body: JSON.stringify(myBody),
		})
		let json = await response.json()
		console.log(json)
	} catch {
		console.log('无法访问 URL')
	}
})
```

fetch 命令返回一个 `response` 对象，包含以下数据：

* `headers`: 一个 `ReadOnlyHeaders` 对象。调用 `get()` 方法来获取某个特定标头，或者调用 `has()` 方法来检查标头是否存在。
* `ok`: 布尔值
* `redirected`: 布尔值
* `status`: 状态码数字
* `statusText`: 状态码文本
* `type`: 将具有以下值之一： *basic*, *cors*, *default*, *error*, *opaque*, *opaqueredirect*
* `url`: 已发送的 URL
* `json()`: 以 JSON 格式获取正文。
* `text()`: 以文本形式获取正文。


> [!WARNING]
> **📔 注意**: `json()` 和 `text()` 二者互斥。如果你以其中一种格式获取响应正文，就无法再从 `response` 对象。



> [!WARNING]
> **📔 注意**: 每个 Decentraland 场景一次只允许执行一个 `fetch` 命令。这对场景代码必须如何组织没有影响，因为请求会在内部排队。如果你的场景需要向不同端点发送多个请求，请记住，每个请求只有在前一个请求得到响应后才会发送。


## 签名请求

你可以采用额外的安全措施来证明请求源自 Decentraland 内的某个玩家会话。你可以发送带有附加签名的请求，该签名使用 Decentraland 会话为每位玩家基于其地址生成的临时密钥进行签署。接收请求的服务器随后可以验证该签名消息确实匹配当前在世界内处于活动状态的地址。

当玩家可能有动机滥用系统、在游戏中刷取代币或积分时，这类安全措施尤其有价值。

要发送签名请求，你只需使用 `signedFetch()` 函数，方式与使用 `fetch()` 函数。

```ts
import { signedFetch } from '~system/SignedFetch'

executeTask(async () => {
	try {
		let response = await signedFetch({
			url: callUrl,
			init: {
				headers: { 'Content-Type': 'application/json' },
				method: 'POST',
				body: JSON.stringify(myBody),
			},
		})

		if (!response.ok) {
			throw new Error('Invalid response')
		}

		let json = JSON.parse(response.body)

		console.log('已收到响应：', json)
	} catch {
		console.log('无法访问 URL')
	}
})
```

请求包含额外的一组标头，其中包含签名消息和一组用于解释该消息的元数据。签名消息由使用玩家临时密钥加密的请求全部内容组成。

该 `signedFetch()` 不同于 `fetch()` 函数，因为响应是完整 HTTP 消息的 Promise，表示为一个 `FlatFetchResponse` 对象。它包括以下属性：

* `body`
* `headers`
* `ok`
* `status`
* `statusText`

该 **body** 始终是一个字符串，你可以像上面的示例那样对其进行解析。

### 验证签名请求

要使用签名请求，接收这些请求的服务器应验证签名是否与请求的其余部分匹配，以及签名消息中编码的时间戳是否为当前时间。

你可以在以下示例场景中找到一个执行此任务的简单服务器示例：

[验证玩家身份真实性](https://github.com/decentraland-scenes/validate-player-authenticity)

## 请求超时

如果 HTTP 请求等待响应的时间过长，它就会失败，以便可以发送其他请求。对于 `fetch()`，默认超时阈值为 30 秒，但你可以通过配置 `timeout` 属性。该值 `timeout` 以毫秒表示。

```ts
fetch('https://some-url.com', { timeout: 1000 })
```

## 使用 WebSockets

你也可以向 WebSocket 服务器发送和获取数据，只要该服务器使用带有 *wss*.

```ts
var socket = new WebSocket('url')

socket.onmessage = function (event) {
	console.log('收到 WebSocket 消息：', event)
}
```

使用 WebSockets 的语法与 JavaScript 原生实现的语法没有区别。有关如何在 WebSockets 上接收和发送消息的详细信息，请参阅 [Mozilla Web API](https://developer.mozilla.org/en-US/docs/Web/API/WebSocket) 文档。


> [!NOTE]
> **💡 提示**: 有一个可简化 websocket 连接使用、并已被证明与 Decentraland 配合良好的库是 [Colyseus](https://colyseus.io/)。另外一些 websocket 库与 Decentraland SDK 不兼容。
> 
> 它在 websocket 连接之上构建了一层抽象，使得响应变化以及在服务器端远程存储一致的游戏状态变得非常简单。你可以在这些示例中看到它的实际效果：
> 
> * [方块跳跃者](https://github.com/decentraland-scenes/cube-jumper-colyesus-sdk7)
> * [太空叛徒](https://github.com/decentraland-scenes/Space-Traitor)
> * [AI NPC](https://github.com/decentraland-scenes/inworld-ai-sdk7)


## 调试网络请求

你可以通过打开调试面板来调试网络请求。

要打开调试面板，你可以点击 ![](/docs/_remote/3a2d6a1437dc36383f2065db.webp) 右上角的图标。然后选择 **Web Requests** 选项卡，并点击 **Open Chrome Devtools**.

这将打开一个新的 Chrome 窗口，并显示 Network 选项卡。

参见 [在预览中调试](/docs/creator/content-creator-zh/chang-jing-sdk7/tiao-shi/debug-in-preview/#web-requests) 以了解更多详情。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/lian-wang/network-connections.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
