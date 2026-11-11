> 了解何时以及如何在场景代码中运行异步函数。

# 异步函数

## 概述

场景中的大部分代码都使用单线程同步运行。这意味着命令会按顺序逐行执行。每个命令都必须先等待前一个命令执行完成，才能开始。

即使是场景系统中的函数也会逐个执行，遵循一个 [优先顺序](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/systems/#system-execution-order).

同步运行代码可确保一致性，因为你总是可以确定代码中的命令执行顺序。

另一方面，场景需要每秒更新多次，以生成下一帧。如果代码中的某一部分响应太慢，那么整个主线程就会被卡住，从而导致帧率下降。

这就是为什么在某些情况下，你会希望某些命令异步运行。这意味着你可以先启动一个任务，而与此同时，场景可以继续执行后续代码。场景仍然在单线程上运行，但在异步任务等待响应时，其余代码不会被阻塞。

这对于依赖外部服务且可能需要时间响应的任务尤其有用，因为你不希望等待响应的这段空闲时间阻塞其他任务。

例如：

* 从 REST API 获取数据时
* 在区块链上执行交易时


> [!WARNING]
> **📔 注意**：请记住，在任务完成执行之前，你的场景可能已经渲染了好几帧。请确保场景代码足够灵活，能够在异步任务完成期间处理中间状态。


## 运行一个异步函数

将任何函数标记为 `异步` ，使其异步运行，在等待期间不会阻塞场景中其余代码。

```ts
// 声明异步函数
async function myAsyncTask() {
	// 运行函数的步骤
}

// 调用异步函数
myAsyncTask()

// 其余代码继续执行
```

## executeTask 函数

该 `executeTask()` 函数会异步执行一个 lambda 函数。 `executeTask()` 使我们能够在同一条语句中声明并执行该函数。

```ts
executeTask(async () => {
	let data = await myAsyncTask()
	console.log(data)
})

// 其余代码继续执行
```

## then 函数

该 `then` 函数接受一个 lambda 函数作为参数，该 lambda 函数只会在前一条语句完成后执行。该 lambda 函数可以选择接收输入，这些输入会映射自前一条语句返回的内容。

```ts
myAsyncTask().then((data) => {
	console.log(data)
})
```


> [!WARNING]
> **📔 注意**：通常最好使用 `executeTask` 方法，而不是 `then` 函数。如果你过度依赖 `then` 函数在多个嵌套层级中工作，你最终可能会陷入所谓的“回调地狱”，代码会变得非常难以阅读和维护。


## PointerEvents 和 RayCast 函数

当你的场景使用一个 `PointerEvent` 或 `RayCast` 组件时，碰撞计算会在引擎中异步进行。随后引擎会向场景返回一个结果事件，该事件可能会比触发该事件时晚一个或几个游戏循环 tick 到达。

然后你需要创建一个系统，在这些结果到达的那一帧中处理它们。


> [!WARNING]
> **📔 注意**：如果你通过以下方式处理点击： [**注册回调**](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/register-callback/) 方法，你无需显式创建系统来处理它，但同样的过程会在后台发生。


参见 [点击事件](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/click-events/) 和 [射线投射](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/raycasting/).


> [!NOTE]
> **💡 提示**：如果射线投射结果的处理需要大量计算（例如运行路径查找算法），你可能希望在异步函数中执行该计算。


## await 语句

一个 `await` 该语句会强制执行等待响应后，才继续到下一行代码。 `await` 语句只能在 async 代码块中使用。

```ts
// 声明函数
async function myAsyncTask() {
	try {
		let response = await fetch(callUrl)
		let json = await response.json()
		console.log(json)
	} catch {
		console.log('无法访问该 URL')
	}
}

// 调用函数
myAsyncTask()

// 其余代码继续执行
```

上面的示例执行了一个包含以下内容的函数： `fetch()` 从外部 API 检索数据的操作。该 `fetch()` 操作是异步的，因为我们无法预测服务器需要多长时间才能响应。不过，下一行需要该操作的输出先准备好，我们才能将其解析为 JSON。该 `await` 此处的语句可确保下一行只会在该操作返回值后才执行。同样地， `response.json()` 函数也是异步的，但下一行需要先解析 JSON 才能将其记录到日志中。第二个 `await` 语句会强制下一行仅在 JSON 解析完成后才调用，无论这需要多长时间。

## 为函数调用设置超时

使用 `setTimeout` 用于在执行某些代码行之前等待一段时间。这需要两个参数：

* 要执行的函数
* 在执行该函数之前要等待的毫秒数

下面的示例会等待 1000 毫秒（即 1 秒）后，再执行一个向控制台输出消息的简单函数。

```ts
import { timers } from '@dcl/sdk/ecs'

console.log('这会立即打印出来')

timers.setTimeout(() => {
	// 延迟后运行的函数
    console.log('这会在 1 秒后打印出来')
}, 1000)
```

该 `clearTimeout` 可用于取消对以下内容的执行： `setTimeout` 仍在等待执行的函数。 `setTimeout` 会返回一个计时器 ID（一个数字），然后你可以将其传递给 `clearTimeout`。在这种情况下，变量 `timeoutId` 是在执行 `setTimeout`时获得的，然后传递给 `clearTimeout` 以取消它。

```ts
import { timers } from '@dcl/sdk/ecs'

const timeoutId = timers.setTimeout(() => {
    console.log('等待 1 秒后再执行此函数')
}, 1000)

timers.clearTimeout(timeoutId)
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/bian-cheng-mo-shi/async-functions.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
