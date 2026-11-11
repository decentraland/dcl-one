# 全局变量

场景运行时会确保在沙箱化的 JavaScript 环境中存在某些对象和函数，此外还提供诸如 `Promise`, `Date` 或 `Math`.


> [!NOTE]
> 下述全局对象是典型的 JavaScript 概念，但它们是为 Decentrland 场景运行时定制的，行为可能与浏览器或 Node 环境中的对应对象并不完全相同。


### 全局对象

运行时会在场景的全局作用域中注入 6 个定义：

1. [`console`](#console)：典型版本的简化版 `console` 对象。
2. [`exports`](#module)：一个对象，场景可以向其中添加其 [公共接口](/docs/contributor/contributor-zh/chang-jing-yun-xing-shi/execution/).
3. [`模块`](#module)：……的容器 `exports` 对象。
4. [`require`](#module)：一个按名称加载运行时提供的模块的函数。
5. [`fetch`](#http)：浏览器的受限实现的 `fetch` 函数。
6. [`WebSocket`](#http)：浏览器的受限实现的 `WebSocket` 类。

所有这些都被定义为只读属性，因此不能重新赋值。除非授予某些 [权限](/docs/contributor/contributor-zh/nei-rong/shi-ti-lei-xing/scenes/#permissions) 权限，否则其中一些在使用时会抛出异常。

### 控制台 <a href="#console" id="console"></a>

场景可以访问一个 `console` 对象，类似于浏览器或 Node 环境提供的对象，但仅限于通常可用的部分方法。

```ts
类型 Console = {
  log(...args: any): void
  info(...args: any): void
  debug(...args: any): void
  warning(...args: any): void
  error(...args: any): void
}
```

与标准对应方法一样，每个方法都接受任意类型的可变参数，并将它们渲染为人类可读的消息。例如，下面这样是有效的：

```js
console.log("东西刚刚出现了", { thing: "foo" }, [1, 2, 3])
```

这些方法的具体行为取决于提供方，但消息必须能让调试代码的场景开发者访问到。


> [!NOTE]
> 在 Foundation 基于浏览器的 World Explorer 中，日志消息会显示在开发者工具面板中。


### 导入和导出 <a href="#module" id="module"></a>

场景可以使用传统的 [CommonJS](https://wiki.commonjs.org/wiki/Modules/1.0) 模块接口来导入和导出对象。

```ts
exports：对象
require(moduleName: string)：对象
```

该 `require` 函数允许场景访问运行时提供的模块（例如 [EngineApi](/docs/contributor/contributor-zh/chang-jing-yun-xing-shi/yun-xing-shi-mo-kuai/engine-api/) 或 [RestrictedActions](/docs/contributor/contributor-zh/chang-jing-yun-xing-shi/yun-xing-shi-mo-kuai/restricted-actions/)），除此之外什么都不能访问（它 **不会** 通过路径访问 NPM 包或模块）。

添加到 `exports` 对象中的属性是场景的公共接口，并会暴露给运行时。事实上，场景 *必须* 至少暴露一个方法才能正常运行（参见 [执行](/docs/contributor/contributor-zh/chang-jing-yun-xing-shi/execution/)).


> [!NOTE]
> 使用 TypeScript 等语言编写的场景会使用更现代的 `import` 和 `export` 语句，它们可以被转译为与 CommonJS 兼容的 `require` 和 `exports`.


### HTTP 和 WebSockets <a href="#http" id="http"></a>

该 `fetch` 和 `WebSocket` 全局对象的工作方式与其众所周知的对应对象完全相同（参见 [fetch](https://developer.mozilla.org/en-US/docs/Web/API/Fetch_API) 和 [WebSocket](https://developer.mozilla.org/en-US/docs/Web/API/WebSockets_API) MDN），但运行时会施加一些限制。

调用 `fetch` 函数时：

* 如果 URL 不以 `https://`.
* 如果场景没有 [`USE_FETCH` 权限](/docs/contributor/contributor-zh/nei-rong/shi-ti-lei-xing/scenes/#permissions).
* 一个由实现定义的超时时间可以中止该请求。

使用 `WebSocket` 类时：

* 如果 URL 不以 `wss:`
* 如果场景没有 [`USE_WEBSOCKET` 权限](/docs/contributor/contributor-zh/nei-rong/shi-ti-lei-xing/scenes/#permissions).

除了这些差异外，两种情况都遵循标准行为。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-zh/chang-jing-yun-xing-shi/globals.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
